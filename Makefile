CC = gcc
CFLAGS = -Wall -Wextra -std=c99 -I./include
LDFLAGS = -L./target/release -lpython_gc -Wl,-rpath,./target/release

.PHONY: all clean test build-rust

all: build-rust test_c_integration

build-rust:
	cargo build --release

test_c_integration: tests/test_c_integration.c
	$(CC) $(CFLAGS) -o test_c_integration tests/test_c_integration.c $(LDFLAGS)

test: test_c_integration
	./test_c_integration

clean:
	rm -f test_c_integration
	cargo clean

build: build-rust test_c_integration

test-all: test
	cargo test

info:
	@echo "Rust library:"
	@ls -la target/release/libpython_gc.*
	@echo ""
	@echo "C header:"
	@ls -la include/python_gc.h
	@echo ""
	@echo "C test program:"
	@ls -la tests/test_c_integration.c 