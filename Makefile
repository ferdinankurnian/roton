.PHONY: dev

dev:
	@command -v watchexec >/dev/null 2>&1 || { \
		printf '%s\n' 'watchexec is required. install it with: sudo pacman -S watchexec'; \
		exit 1; \
	}
	watchexec --restart --clear \
		--watch src \
		--watch assets \
		--watch Cargo.toml \
		--watch Cargo.lock \
		--exts rs,toml,lock,svg,png \
		-- cargo run
