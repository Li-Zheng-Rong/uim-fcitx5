PREFIX ?= /usr/local
UIM_LIBDIR = $(shell pkg-config --variable=libdir uim)
UIM_SCMDIR = $(shell pkg-config --variable=uim_scmdir uim)

.PHONY: all install uninstall

all:
	cargo build --release --locked

install:
	install -Dm755 target/release/libuim_fcitx5.so "$(DESTDIR)$(UIM_LIBDIR)/uim/plugin/libuim-fcitx5.so"
	install -Dm644 scm/fcitx5.scm "$(DESTDIR)$(UIM_SCMDIR)/fcitx5.scm"
	install -Dm755 bin/uim-fcitx5 "$(DESTDIR)$(PREFIX)/bin/uim-fcitx5"
	@if [ -z "$(DESTDIR)" ]; then uim-module-manager --register fcitx5; fi

uninstall:
	@if [ -z "$(DESTDIR)" ]; then uim-module-manager --unregister fcitx5; fi
	rm "$(DESTDIR)$(UIM_LIBDIR)/uim/plugin/libuim-fcitx5.so" "$(DESTDIR)$(UIM_SCMDIR)/fcitx5.scm" "$(DESTDIR)$(PREFIX)/bin/uim-fcitx5"
