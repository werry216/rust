# This file is generated by gyp; do not edit.

export builddir_name ?= mk/libuv/mips/unix/linux/./src/libuv/out
.PHONY: all
all:
	$(MAKE) -C ../.. uv run-benchmarks run-tests
