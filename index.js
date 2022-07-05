const { PORT } = require("./config/constants/general");
const express = require("express");
const app = express();
const dotenv = require("dotenv");
const mongoose = require("mongoose");
const cors = require("cors");
const helmet = require("helmet");
const rateLimit = require("express-rate-limit");
const compression = require("compression");

// Routes
const authRoute = require("./routes/auth");
const postsRoute = require("./routes/posts");
const searchRoute = require("./routes/search");

dotenv.config({ path: "./config/.env" });

mongoose.connect(process.env.DB_CONNECT, () => {
  console.log("Connected to DB");
});

// Middlewares
app.use(express.json());
app.use(helmet());
app.use(compression());
app.use(
  rateLimit({
    // In the future, rate limit specific routes
    windowMs: 20 * 60 * 1000, // 20 minutes
    max: 10000, // x requests per 20 minutes (CHANGE THIS LATER)
  })
);

// Cors
app.use(cors({ origin: "*" })); // Frontend address this API can be called from, in the future, make the specific frontend app address?

// Error
app.use((err, req, res, next) => {
  if (err) res.status(500).send({ msg: "Internal server error: 500" });
  else next();
});

// Routes
app.use("/api/user", authRoute);
app.use("/api/posts", postsRoute);
app.use("/api/search", searchRoute);

app.listen(PORT, () => {
  console.log(`Server running on port ${PORT}`);
});
