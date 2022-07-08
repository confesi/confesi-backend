const University = require("../models/University");
const User = require("../models/User");

// SEARCH UP USERS //

const users = async (req, res) => {
  // Deconstructs passed data
  const { username } = req.body;

  // Makes sure required data is passed
  if (username == null)
    return res.status(400).json({ error: "fields cannot be blank" });

  // An aggregation value that allows us to fuzzy search
  // for usernames in the User's collection
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

  try {
    // Runs the pipeline using our aggregation.
    const result = await User.aggregate(agg);
    return res.status(200).json({ users: result });
  } catch (error) {
    // Server error searching for users
    return res.status(500).json({ error: "Internal server error" });
  }
};

// SEARCH UP UNIVERSITIES //

const universities = async (req, res) => {
  // Deconstructs passed data
  const { university } = req.body;

  // Makes sure required data is passed
  if (university == null)
    return res.status(400).json({ error: "fields cannot be blank" });

  // An aggregation value that allows us to fuzzy search
  // for universities by their extended names in the User's collection
  // "Extended names" = "University of Victoria" != "UVic"
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

  try {
    // Runs the pipeline using our aggregation.
    const result = await University.aggregate(agg);
    return res.status(200).json({ universities: result });
  } catch (error) {
    // Server error searching for universities
    return res.status(500).json({ error: "Internal server error" });
  }
};

module.exports = { users, universities };
