.PHONY: test fmt clippy rust-test npm-test deny licenses package-check pty-smoke

test: fmt clippy rust-test npm-test deny

fmt:
	cargo fmt --check

clippy:
	cargo clippy --locked --all-targets --all-features -- -D warnings

rust-test:
	cargo test --locked --all-targets --all-features

npm-test:
	npm test

deny:
	cargo deny check

licenses:
	cargo about generate about.hbs -o THIRD_PARTY_LICENSES.html

package-check: test licenses
	node scripts/verify-package.mjs --check-license-tree
	cargo publish --dry-run --locked
	npm run package-check

pty-smoke:
	cargo build --release --locked
	python3 scripts/pty-smoke.py target/release/typeul
