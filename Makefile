NAME:= $(shell basename `pwd`)

all: run

edit:
	@vim -c 'set nu et bg=dark' src/main.rs

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

.PHONY: all edit run check test release
