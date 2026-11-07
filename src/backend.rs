//! Fcitx5's public D-Bus frontend, driven from the UIM main thread.
//!
//! Ordinary ProcessKeyEvent keeps commits and both UI signals on the same
//! ordered stream. ProcessKeyEventBatch cannot do that: it batches client
//! preedit and commits, while client-side input-panel updates remain signals.

use std::os::fd::RawFd;
use std::time::Duration;

use dbus::arg::{AppendAll, ReadAll};
use dbus::channel::{BusType, Channel};
use dbus::{Error, Message, MessageType, Path};

use crate::protocol::{Event, KeyResult};

pub type Result<T> = std::result::Result<T, Error>;

const SERVICE: &str = "org.freedesktop.portal.Fcitx";
const INPUT_METHOD_PATH: &str = "/org/freedesktop/portal/inputmethod";
const INPUT_METHOD_INTERFACE: &str = "org.fcitx.Fcitx.InputMethod1";
const INPUT_CONTEXT_INTERFACE: &str = "org.fcitx.Fcitx.InputContext1";
const CONTROLLER_SERVICE: &str = "org.fcitx.Fcitx5";
const CONTROLLER_PATH: &str = "/controller";
const CONTROLLER_INTERFACE: &str = "org.fcitx.Fcitx.Controller1";
const CALL_TIMEOUT: Duration = Duration::from_secs(5);

// Values from fcitx-utils/capabilityflags.h. SurroundingText is deliberately
// absent: UIM FEP does not expose the application's surrounding text.
const PREEDIT: u64 = 1 << 1;
const FORMATTED_PREEDIT: u64 = 1 << 4;
const NO_ON_SCREEN_KEYBOARD: u64 = 1 << 15;
const GET_IM_INFO_ON_FOCUS: u64 = 1 << 23;
const TERMINAL: u64 = 1 << 32;
const CLIENT_SIDE_INPUT_PANEL: u64 = 1 << 39;
const CAPABILITIES: u64 = PREEDIT
    | FORMATTED_PREEDIT
    | NO_ON_SCREEN_KEYBOARD
    | GET_IM_INFO_ON_FOCUS
    | TERMINAL
    | CLIENT_SIDE_INPUT_PANEL;

pub struct Backend {
    channel: Channel,
    path: Path<'static>,
    owner: String,
}

impl Backend {
    pub fn new() -> Result<Self> {
        let mut channel = Channel::get_private(BusType::Session)?;
        channel.set_watch_enabled(true);
        // Only the standard name has a D-Bus activation service. Its first
        // method reply also waits for Fcitx's startup addon loading to finish,
        // including registration of the portal name and frontend objects.
        let (_state,): (i32,) = Self::call_on(
            &channel,
            CONTROLLER_SERVICE,
            CONTROLLER_PATH,
            CONTROLLER_INTERFACE,
            "State",
            (),
        )?;
        let (owner,): (String,) = Self::call_on(
            &channel,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetNameOwner",
            (SERVICE,),
        )?;
        let args = vec![("program", "uim-fep"), ("display", "tty:")];
        let (path, _uuid): (Path<'static>, Vec<u8>) = Self::call_on(
            &channel,
            &owner,
            INPUT_METHOD_PATH,
            INPUT_METHOD_INTERFACE,
            "CreateInputContext",
            (args,),
        )?;
        let backend = Self {
            channel,
            path,
            owner,
        };
        let input_context_rule = format!(
            "type='signal',sender='{}',path='{}',interface='{}'",
            backend.owner, backend.path, INPUT_CONTEXT_INTERFACE
        );
        let owner_rule = format!(
            "type='signal',sender='org.freedesktop.DBus',path='/org/freedesktop/DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='{}'",
            backend.owner
        );
        for rule in [input_context_rule, owner_rule] {
            let (): () = Self::call_on(
                &backend.channel,
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "AddMatch",
                (rule,),
            )?;
        }
        backend.call::<_, ()>("SetSupportedCapability", (CAPABILITIES,))?;
        backend.call::<_, ()>("SetCapability", (CAPABILITIES,))?;
        Ok(backend)
    }

    /// The connection stays on this fd for its lifetime.
    pub fn fd(&self) -> RawFd {
        self.channel.watch().fd
    }

    /// Refresh after every operation: queued output needs writable readiness
    /// until libdbus has flushed it, including a final key release at idle.
    pub fn watch_flags(&self) -> u32 {
        let watch = self.channel.watch();
        u32::from(watch.read) | (u32::from(watch.write) << 1)
    }

    /// Drain libdbus's existing queue as well as newly readable socket data.
    /// A blocking call can have already pulled signals off the socket, so
    /// consumers call this after every operation as well as on fd readiness.
    pub fn dispatch(&mut self) -> Result<Vec<Event>> {
        let (mut events, error) = self.drain_events();
        if let Some(error) = error {
            events.push(Event::Error(error.to_string()));
        }
        Ok(events)
    }

    fn drain_events(&self) -> (Vec<Event>, Option<Error>) {
        let mut events = Vec::new();
        let mut error = None;
        self.drain_queued(&mut events, &mut error);
        if self.channel.read_write(Some(Duration::ZERO)).is_err() {
            error.get_or_insert_with(|| Error::new_failed("Fcitx5 D-Bus connection closed"));
        }
        self.drain_queued(&mut events, &mut error);
        (events, error)
    }

    fn drain_queued(&self, events: &mut Vec<Event>, error: &mut Option<Error>) {
        while let Some(message) = self.channel.pop_message() {
            match self.event(&message) {
                Ok(Some(Event::Error(message))) => {
                    error.get_or_insert_with(|| Error::new_failed(&message));
                }
                Ok(Some(event)) => events.push(event),
                Ok(None) => {}
                Err(cause) => {
                    error.get_or_insert(cause);
                }
            }
        }
    }

    fn failed_call(&self, method: &str, cause: Error) -> Vec<Event> {
        // The server can commit before a method reply is lost. Preserve those
        // signals before reporting failure and letting Scheme close the IC.
        let (mut events, _) = self.drain_events();
        events.push(Event::Error(format!(
            "Fcitx5 {method} result unavailable; request not replayed: {cause}"
        )));
        events
    }

    pub fn focus_in(&mut self) -> Result<Vec<Event>> {
        self.action("FocusIn", ())
    }

    pub fn focus_out(&mut self) -> Result<Vec<Event>> {
        self.action("FocusOut", ())
    }

    pub fn reset(&mut self) -> Result<Vec<Event>> {
        self.action("Reset", ())
    }

    pub fn select_candidate(&mut self, index: i32) -> Result<Vec<Event>> {
        self.action("SelectCandidate", (index,))
    }

    /// Controller.Toggle targets the most recently focused input context.
    /// An already focused context ignores FocusIn, so cycle our focus first
    /// to refresh its position after activity in another display's group.
    /// Keep the normal focus-out composition events queued for dispatch.
    /// This also supports users with no configured trigger key.
    pub fn toggle(&mut self) -> Result<Vec<Event>> {
        for method in ["FocusOut", "FocusIn"] {
            if let Err(error) = self.call::<_, ()>(method, ()) {
                return Ok(self.failed_call(method, error));
            }
        }
        if let Err(error) = Self::call_on::<_, ()>(
            &self.channel,
            CONTROLLER_SERVICE,
            CONTROLLER_PATH,
            CONTROLLER_INTERFACE,
            "Toggle",
            (),
        ) {
            return Ok(self.failed_call("Toggle", error));
        }
        self.dispatch()
    }

    pub fn process_key(&mut self, keysym: u32, state: u32, release: bool) -> Result<KeyResult> {
        // UIM supplies logical keys without a hardware keycode or timestamp.
        if release {
            // Terminal releases carry no bytes and need no filtering decision.
            // Send on the same channel to retain press/release/key ordering;
            // dispatch and writable watches finish any buffered output.
            let result = Self::message(
                &self.owner,
                &self.path,
                INPUT_CONTEXT_INTERFACE,
                "ProcessKeyEvent",
                (keysym, 0_u32, state, true, 0_u32),
            )
            .and_then(|mut message| {
                message.set_no_reply(true);
                self.channel
                    .send(message)
                    .map(|_| ())
                    .map_err(|()| Error::new_failed("Unable to queue Fcitx5 key release"))
            });
            return Ok(KeyResult {
                handled: true,
                events: match result {
                    Ok(()) => self.dispatch()?,
                    Err(error) => self.failed_call("key release send", error),
                },
            });
        }
        let handled = match self
            .call::<_, (bool,)>("ProcessKeyEvent", (keysym, 0_u32, state, false, 0_u32))
        {
            Ok((handled,)) => handled,
            Err(error) => {
                return Ok(KeyResult {
                    // A missing reply cannot authorize forwarding the key to
                    // the child: Fcitx may already have consumed it.
                    handled: true,
                    events: self.failed_call("ProcessKeyEvent", error),
                });
            }
        };
        Ok(KeyResult {
            handled,
            events: self.dispatch()?,
        })
    }

    pub fn shutdown(&mut self) -> Result<Vec<Event>> {
        self.action("DestroyIC", ())
    }

    fn action<A: AppendAll>(&mut self, method: &str, args: A) -> Result<Vec<Event>> {
        match self.call::<_, ()>(method, args) {
            Ok(()) => self.dispatch(),
            Err(error) => Ok(self.failed_call(method, error)),
        }
    }

    fn call<A: AppendAll, R: ReadAll>(&self, method: &str, args: A) -> Result<R> {
        Self::call_on(
            &self.channel,
            &self.owner,
            &self.path,
            INPUT_CONTEXT_INTERFACE,
            method,
            args,
        )
    }

    fn call_on<A: AppendAll, R: ReadAll>(
        channel: &Channel,
        destination: &str,
        path: &str,
        interface: &str,
        method: &str,
        args: A,
    ) -> Result<R> {
        channel
            .send_with_reply_and_block(
                Self::message(destination, path, interface, method, args)?,
                CALL_TIMEOUT,
            )?
            .read_all()
    }

    fn message<A: AppendAll>(
        destination: &str,
        path: &str,
        interface: &str,
        method: &str,
        args: A,
    ) -> Result<Message> {
        let mut message = Message::new_method_call(destination, path, interface, method)
            .map_err(|error| Error::new_failed(&error))?;
        message.append_all(args);
        Ok(message)
    }

    fn event(&self, message: &Message) -> Result<Option<Event>> {
        if message.msg_type() == MessageType::Signal
            && message.sender().as_deref() == Some("org.freedesktop.DBus")
            && message.interface().as_deref() == Some("org.freedesktop.DBus")
            && message.member().as_deref() == Some("NameOwnerChanged")
        {
            let (name, _, owner): (String, String, String) = message.read3()?;
            if name == self.owner && owner.is_empty() {
                return Ok(Some(Event::Error("Fcitx5 service disconnected".into())));
            }
        }
        if message.msg_type() != MessageType::Signal
            || message.path().as_deref() != Some(self.path.as_ref())
            || message.interface().as_deref() != Some(INPUT_CONTEXT_INTERFACE)
            || message.sender().as_deref() != Some(self.owner.as_str())
        {
            return Ok(None);
        }
        let event = match message.member().as_deref() {
            Some("CommitString") => Event::Commit(message.read1()?),
            Some("UpdateFormattedPreedit") => {
                let (text, cursor) = message.read2()?;
                Event::Preedit { text, cursor }
            }
            Some("UpdateClientSideUI") => {
                let mut args = message.iter_init();
                let preedit = args.read()?;
                let cursor = args.read()?;
                let aux_up = args.read()?;
                let aux_down = args.read()?;
                let candidates = args.read()?;
                let candidate_index = args.read()?;
                // UIM FEP uses its terminal candidate layout.
                let _layout_hint: i32 = args.read()?;
                Event::ClientSideUI {
                    preedit,
                    cursor,
                    aux_up,
                    aux_down,
                    candidates,
                    candidate_index,
                    has_prev: args.read()?,
                    has_next: args.read()?,
                }
            }
            Some("ForwardKey") => {
                let (keysym, state, release) = message.read3()?;
                Event::ForwardKey {
                    keysym,
                    state,
                    release,
                }
            }
            Some("DeleteSurroundingText") => {
                let (offset, nchars) = message.read2()?;
                Event::DeleteSurrounding { offset, nchars }
            }
            Some("CurrentIM") => {
                let (name, unique_name, _language): (String, String, String) = message.read3()?;
                Event::CurrentIM { name, unique_name }
            }
            Some("NotifyFocusOut") => Event::FocusOut,
            _ => return Ok(None),
        };
        Ok(Some(event))
    }
}
