## Confesi - Server Repo (NodeJS)
### Installing and running the project
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
#### Initial setup
 1. Go into the directory of the server (you just cloned it) by running `cd confessi-server`.
 2. Run the command `npm i` to install the project's node modules.
 3. Go into the `config` directory by running the command `cd config`.
 4. Create the `.env` file by running the command: `touch .env` (create file however you do on your shell, example is with git bash).
 5. Contact the repository owner for the secret content of the `.env` file. This is critical to the server running.
#### Running the project
 1. Inside the `confessi-server` directory, run the command `npm start`. This command will start [nodemon](https://www.npmjs.com/package/nodemon).
 2. The server is now up and running!
