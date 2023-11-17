net_server_coord:
	RUST_LOG=debug cargo run --bin russula -- --protocol NetbenchServerCoordinator
net_server_worker:
	RUST_LOG=debug cargo run --bin russula -- --protocol NetbenchServerWorker

test_server:
	RUST_LOG=debug cargo test -- server --nocapture
test_client:
	RUST_LOG=debug cargo test -- client --nocapture

# net_client_worker:
# 	cargo run --bin russula -- --protocol NetbenchClientWorker

