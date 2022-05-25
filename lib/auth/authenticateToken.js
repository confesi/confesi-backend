const jwt = require('jsonwebtoken');

function authenticateToken(req, res, next) {
    const authHeader = req.headers["authorization"];
    const token = authHeader && authHeader.split(" ")[1];
    if (token == null) return res.status(401).send("No access token provided");

    jwt.verify(token, process.env.ACCESS_TOKEN_SECRET, (e, user) => {
        if (e) return res.status(403).send("Not authorized");
        req.user = user
        next()
    })
}

module.exports = authenticateToken;