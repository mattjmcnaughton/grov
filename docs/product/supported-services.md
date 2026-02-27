# Supported Services

| Service | Docker | Native (Linux only) | Notes |
|---|---|---|---|
| **PostgreSQL** | ✓ | ✓ | `initdb` for isolated clusters |
| **MinIO** | ✓ | ✓ | Positional arg for data dir |

Adding new services is composable — any Docker image or binary that accepts a port flag can be configured.

## On Deck

| Service | Notes |
|---|---|
| **Temporal** | Multi-service orchestration (server + dependencies), high local setup complexity |
| **Redis** | `--dir` for data isolation |
| **DynamoDB Local** | `-dbPath` + `-sharedDb`, needs JRE |
| **MySQL/MariaDB** | `--datadir` for data isolation |
| **Kafka** | KRaft mode removes ZooKeeper dependency, `--log-dirs` for data isolation |
| **Elasticsearch** | `path.data` for data isolation |
| **RabbitMQ** | `RABBITMQ_MNESIA_DIR` for data isolation |
