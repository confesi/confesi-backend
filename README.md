### Setting up the project

1. Run `docker compose up -d mongo` to start the database server for the first time.
2. Run `docker compose run --rm mongo mongosh mongodb://mongo/db --eval 'rs.initiate()'` to initialize the replica set.


### Running the project

Run `docker compose up --build app` to start the app.


### Starting fresh

Sometimes, backwards-incompatible changes during early development will make it necessary to restart from an empty database.

Run `docker compose down --remove-orphans --volumes` to remove all data, then start at **Setting up the project** again.


### API documentation

The API is documented in [docs/openapi.yaml](docs/openapi.yaml) and can be browsed at <http://api-docs.localhost:8080/> after starting the documentation server with `docker compose up --build docs`.
