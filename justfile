build: check fmt-fix lint-fix test
	cargo build

ci: check fmt-check lint-check test
	cargo build --release

clean:
	cargo clean

test:
	cargo test

clippy_args := """
	-D clippy::all \
	-D clippy::pedantic \
	-D clippy::nursery \
	-D clippy::cargo \
	-D clippy::mod-module-files \
	-D clippy::allow_attributes_without_reason \
	-D clippy::as_conversions \
	-D clippy::missing_assert_message \
	-A clippy::multiple-crate-versions \
	-A clippy::arithmetic_side_effects \
	-A clippy::integer_division \
	-A clippy::float_arithmetic \
	-A clippy::cast_precision_loss \
	-A clippy::missing-docs-in-private-items \
	-A clippy::implicit_return \
	-A clippy::separated_literal_suffix \
	-A clippy::std_instead_of_core \
	-A clippy::mod_module_files \
	-A clippy::option_if_let_else \
	-A clippy::missing_trait_methods \
	-A clippy::used_underscore_binding \
	-A clippy::future_not_send \
	\
	-A clippy::similar_names \
	-A clippy::needless_pass_by_value
	"""

lint-check:
	cargo clippy -- {{clippy_args}}

lint-fix:
	cargo clippy --allow-dirty --allow-staged --fix -- {{clippy_args}}

fmt-check: rust-fmt-check toml-fmt-check md-fmt-check

rust-fmt-check:
	cargo fmt --check

toml-fmt-check:
	taplo fmt --check *.toml

md-fmt-check:
	markdownlint *.md --config .markdownlint.jsonc


fmt-fix: rust-fmt-fix toml-fmt-fix md-fmt-fix

rust-fmt-fix:
	cargo fmt

toml-fmt-fix:
	taplo fmt *.toml

md-fmt-fix:
	markdownlint *.md --config .markdownlint.jsonc --fix

check:
	cargo check

run:
	cargo run
