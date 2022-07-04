const University = require("../models/University");
const User = require("../models/User");

const users = async (req, res) => {
  console.log("<=== SEARCH USERS ROUTE HIT ===>");

  // Deconstructs passed data
  const { username } = req.body;

  // Makes sure required data is passed
  if (!username)
    return res.status(400).json({ error: "fields cannot be blank" });

  const agg = [
    {
      $search: {
        autocomplete: {
          query: username,
          path: "username",
          fuzzy: { maxEdits: 1 },
        },
        index: "user_search_index",
      },
    },
    { $limit: 7 },
    {
      $project: {
        _id: 0,
        username: 1,
        display_name: 1,
        score: { $meta: "searchScore" },
      },
    },
    { $sort: { score: -1 } },
  ];

  // run pipeline
  const result = await User.aggregate(agg);
  return res.status(200).json({ users: result });
};

const universities = async (req, res) => {
  console.log("<=== SEARCH UNIVERSITIES ROUTE HIT ===>");

  // Deconstructs passed data
  const { university } = req.body;

  // Makes sure required data is passed
  if (!university)
    return res.status(400).json({ error: "fields cannot be blank" });

  const agg = [
    {
      $search: {
        autocomplete: {
          query: university,
          path: "name",
          fuzzy: { maxEdits: 1 },
        },
        index: "university_search_index",
      },
    },
    { $limit: 7 },
    {
      $project: {
        _id: 0,
        name: 1,
        school_code: 1,
        score: { $meta: "searchScore" },
      },
    },
    { $sort: { score: -1 } },
  ];

  // Run pipeline
  const result = await University.aggregate(agg);
  return res.status(200).json({ universities: result });
};

module.exports = { users, universities };
