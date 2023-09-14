test:
	@cargo test -- --nocapture

format:
	@cargo clippy --fix --allow-dirty --allow-staged
	@cargo fmt --all

install: test
	@./install.sh
