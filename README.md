## Confesi - Server Repo (NodeJS)
### Installing the project
#### Fork the repository
 1. Create a Github account.
 2. Go to https://github.com/mattrltrent/confessi-server
 3. Go to the `main` branch and click the "Fork" button on the top right corner.
 4. This will allow you to have your own copy of the project.
#### Clone your fork to local machine
 1. Open the directory on your computer where you want to put the code. For example, `mkdir project`.
 2. Go into your newly created directory: `cd project`.
 3. Open your newly forked repository on Github.
 4. Click "Clone or Download" and copy the url.
 5. Run the command `git clone <COPIED_URL_HERE>` in your `project` directory. This will download the project to your local machine.
#### Create secrets
 1. Inside the `confessi-server` directory, run `docker compose run --rm create-secrets > .env`.
### Running the project
 1. Inside the `confessi-server` directory, run the command `docker compose up --build app`.
 2. The server is now up and running!
