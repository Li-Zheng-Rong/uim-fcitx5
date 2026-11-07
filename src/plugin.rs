//! Native UIM engine entry points. All calls run on the UIM/FEP main thread.

mod events;

use std::ffi::CStr;

use crate::backend::{Backend, Result};
use crate::keymap::{self, Key};
use crate::scheme_ffi::{self as scm, Value};

fn backend(handle: Value) -> &'static mut Backend {
    // Scheme owns the opaque handle until close/abandon and serializes calls.
    unsafe { &mut *scm::uim_scm_c_ptr(handle).cast::<Backend>() }
}

fn output(result: Result<Vec<crate::protocol::Event>>) -> Value {
    match result {
        Ok(events) => events::convert(events, None).0.value(),
        Err(error) => scm::list([events::error(&error.to_string())]).value(),
    }
}

extern "C" fn open() -> Value {
    match Backend::new() {
        Ok(backend) => unsafe { scm::uim_scm_make_ptr(Box::into_raw(Box::new(backend)).cast()) },
        Err(error) => scm::text(&error.to_string()).value(),
    }
}

extern "C" fn fd(handle: Value) -> Value {
    scm::integer(backend(handle).fd().into()).value()
}

extern "C" fn watch_flags(handle: Value) -> Value {
    scm::integer(backend(handle).watch_flags().into()).value()
}

extern "C" fn dispatch(handle: Value) -> Value {
    output(backend(handle).dispatch())
}

extern "C" fn focus_in(handle: Value) -> Value {
    output(backend(handle).focus_in())
}

extern "C" fn focus_out(handle: Value) -> Value {
    output(backend(handle).focus_out())
}

extern "C" fn reset(handle: Value) -> Value {
    output(backend(handle).reset())
}

extern "C" fn select_candidate(handle: Value, index: Value) -> Value {
    output(backend(handle).select_candidate(unsafe { scm::uim_scm_c_int(index) } as i32))
}

extern "C" fn toggle(handle: Value) -> Value {
    output(backend(handle).toggle())
}

extern "C" fn close(handle: Value) -> Value {
    let mut backend = unsafe { Box::from_raw(scm::uim_scm_c_ptr(handle).cast::<Backend>()) };
    unsafe { scm::uim_scm_nullify_c_ptr(handle) };
    output(backend.shutdown())
}

extern "C" fn abandon(handle: Value) -> Value {
    unsafe {
        drop(Box::from_raw(scm::uim_scm_c_ptr(handle).cast::<Backend>()));
        scm::uim_scm_nullify_c_ptr(handle);
    }
    scm::boolean(false).value()
}

extern "C" fn process_key(handle: Value, key: Value, state: Value, release: Value) -> Value {
    let key = if unsafe { scm::uim_scm_intp(key) } != 0 {
        Key::Character(unsafe { scm::uim_scm_c_int(key) } as u32)
    } else {
        let name = unsafe { CStr::from_ptr(scm::uim_scm_refer_c_str(key)) };
        Key::Named(name.to_str().expect("UIM key symbols are ASCII"))
    };
    let Some(key) = keymap::translate(key, unsafe { scm::uim_scm_c_int(state) } as u32) else {
        return scm::list([scm::boolean(false)]).value();
    };
    let release = unsafe { scm::uim_scm_c_bool(release) } != 0;
    let result = if key.keysym == keymap::F13 && key.state == 0 {
        if release {
            return scm::list([scm::boolean(true)]).value();
        }
        backend(handle)
            .toggle()
            .map(|events| crate::protocol::KeyResult {
                handled: true,
                events,
            })
    } else {
        backend(handle).process_key(key.keysym, key.state, release)
    };
    match result {
        Ok(result) => {
            let (events, forwarded) = events::convert(result.events, Some((key, release)));
            scm::pair(&scm::boolean(result.handled && !forwarded), &events).value()
        }
        Err(error) => {
            // A failed reply does not prove the server did not consume the key.
            // Report the uncertainty; never replay this key through the child PTY.
            scm::list([
                scm::boolean(true),
                events::error(&format!("Key result unknown: {error}")),
            ])
            .value()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn uim_dynlib_instance_init() {
    unsafe {
        scm::uim_scm_init_proc0(c"fcitx5-native-open".as_ptr(), open);
        scm::uim_scm_init_proc1(c"fcitx5-native-fd".as_ptr(), fd);
        scm::uim_scm_init_proc1(c"fcitx5-native-watch-flags".as_ptr(), watch_flags);
        scm::uim_scm_init_proc1(c"fcitx5-native-dispatch".as_ptr(), dispatch);
        scm::uim_scm_init_proc1(c"fcitx5-native-focus-in".as_ptr(), focus_in);
        scm::uim_scm_init_proc1(c"fcitx5-native-focus-out".as_ptr(), focus_out);
        scm::uim_scm_init_proc1(c"fcitx5-native-reset".as_ptr(), reset);
        scm::uim_scm_init_proc1(c"fcitx5-native-toggle".as_ptr(), toggle);
        scm::uim_scm_init_proc1(c"fcitx5-native-close".as_ptr(), close);
        scm::uim_scm_init_proc1(c"fcitx5-native-abandon".as_ptr(), abandon);
        scm::uim_scm_init_proc2(c"fcitx5-native-select-candidate".as_ptr(), select_candidate);
        scm::uim_scm_init_proc4(c"fcitx5-native-key".as_ptr(), process_key);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn uim_dynlib_instance_quit() {}
