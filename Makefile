.PHONY: build build-all release test test-ci clippy fmt check run debug repin clean \
       release-macos-arm release-macos-x86 release-linux-arm release-linux-x86 release-windows

# ── Build ────────────────────────────────────────────────────────────────────

build:
	bazel build //:loopal

build-all:
	bazel build //...

release:
	bazel build //:loopal -c opt

# ── Test ─────────────────────────────────────────────────────────────────────

test:
	bazel test //...

test-ci:
	bazel test //... --config=ci

# ── Code Quality ─────────────────────────────────────────────────────────────

clippy:
	bazel build //... --config=clippy

fmt:
	bazel build //... --config=rustfmt

check: clippy fmt test

# ── Run ──────────────────────────────────────────────────────────────────────

MODEL ?= claude-opus-4-6

run:
	bazel run //:loopal -- -m $(MODEL) $(ARGS)

debug:
	LOOPAL_LOG=debug bazel run //:loopal -- -m $(MODEL) $(ARGS)

# ── Dependencies ─────────────────────────────────────────────────────────────

repin:
	CARGO_BAZEL_REPIN=1 bazel sync --only=crates

# ── Cross Compile ────────────────────────────────────────────────────────────

release-macos-arm:
	bazel build //:loopal -c opt --config=macos-arm

release-macos-x86:
	bazel build //:loopal -c opt --config=macos-x86

release-linux-arm:
	bazel build //:loopal -c opt --config=linux-arm

release-linux-x86:
	bazel build //:loopal -c opt --config=linux-x86

release-windows:
	bazel build //:loopal -c opt --config=windows-x86

# ── Clean ────────────────────────────────────────────────────────────────────

clean:
	bazel clean
