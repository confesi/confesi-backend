### Setting up the project

1. Run `docker compose up -d mongo` to start the database server for the first time.
2. To initialize the replica set, run

   ```shell
   docker compose run --rm mongo mongosh mongodb://mongo/db --eval '
     rs.initiate({
       _id: "rs0",
       members: [
         {_id: 0, host: "127.0.0.1"},
       ],
     })
   '
   ```

### Running the project

Run `docker compose up --build app` to start the app.

### Starting fresh

Sometimes, backwards-incompatible changes during early development will make it necessary to restart from an empty database.

Run `docker compose down --remove-orphans --volumes` to remove all data, then start at **Setting up the project** again.

### Viewing MongoDB data via shell

Viewing the raw data can be helpful in development.

1. Run `docker ps` to see all containers.
2. Copy the `CONTAINER ID` from the listed `mongo:5` container.
3. Run `docker exec -it NAME_OF_CONTAINER bash` to open a bash shell inside the mongo container.
4. Run `mongosh` to start the MongoDB CLI tool.
5. You can now run commands like `show dbs`, `use db`, `db.posts.find()`, etc.

### API documentation

The API is documented in [docs/openapi.yaml](docs/openapi.yaml) and can be browsed at <http://api-docs.localhost:8080/> after starting the documentation server with `docker compose up --build docs`.
