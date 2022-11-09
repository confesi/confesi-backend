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


### API documentation

The API is documented in [docs/openapi.yaml](docs/openapi.yaml) and can be browsed at <http://api-docs.localhost:8080/> after starting the documentation server with `docker compose up --build docs`.
