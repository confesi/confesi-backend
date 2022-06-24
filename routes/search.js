const router = require("express").Router();
const { ObjectId } = require("mongodb");
const authenticateToken = require("../lib/auth/authenticateToken");
const User = require("../models/User");

router.get("/users", authenticateToken, async (req, res) => {

    // Deconstructs passed data
    const { username } = req.body;

    // Makes sure required data is passed
    if (!username) return res.status(400).json({"error": "fields cannot be blank"});

    const agg = [
        {$search: {text: {query: username, path: "username", fuzzy: {maxEdits: 2}}, index: "user_search_index"}},
        {$limit: 5},
        {$project: {_id: 0, username: 1, display_name: 1, score: {$meta: "searchScore"}}},
        {$sort: {score: -1}},
    ];
    // run pipeline
    const result = await User.aggregate(agg);
    return res.status(200).json({"users": result});
});

module.exports = router; 