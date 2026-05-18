BINARY  := pmsi2json
DESTDIR := /usr/local/bin

.PHONY: build install uninstall

build:
	cargo build --release

install: build
	install -o root -g root -m 755 target/release/$(BINARY) $(DESTDIR)/$(BINARY)

uninstall:
	rm -f $(DESTDIR)/$(BINARY)
