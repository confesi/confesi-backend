const { PORT } = require("./constants/setup");
const express = require("express");
const app = express();
const dotenv = require("dotenv");
const mongoose = require("mongoose");
const cors = require("cors");

// Routes
const authRoute = require("./routes/auth");
const postsRoute = require("./routes/posts");
const searchRoute = require("./routes/search");

dotenv.config();

mongoose.connect(process.env.DB_CONNECT, () => {console.log("Connected to DB")});

// Cors
app.use(cors({
    // Frontend address this API can be called from
    origin: "*",
}));

// Middlewares
app.use(express.json());
app.use("/api/user", authRoute);
app.use("/api/posts", postsRoute);
app.use("/api/search", searchRoute);






app.listen(PORT, () => {console.log(`Server running on port ${PORT}`)});