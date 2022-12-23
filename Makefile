NAME:= $(shell basename `pwd`)

all: run

edit:
	@vim -c 'set nu et bg=dark' src/main.rs

edit_m:
	@vim -c 'set nu et bg=dark' macro_leon/src/lib.rs

run:
	@cargo run

check:
	@cargo check

test:
	@cargo test

release:
	@cargo build --release &&\
		strip "target/release/$(NAME)" &&\
		ln -f "target/release/$(NAME)" "$(NAME)"

.PHONY: all edit edit_m run check test release
