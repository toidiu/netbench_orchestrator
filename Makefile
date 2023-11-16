net_server_coord:
	cargo run --bin russula -- --protocol NetbenchServerWorker --port 8991

net_client_worker:
	cargo run --bin russula -- --protocol NetbenchClientWorker --port 8991

