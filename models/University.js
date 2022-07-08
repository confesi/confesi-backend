const mongoose = require("mongoose");

// Schema design for universities the app currently supports. This
// collection is queried when the user is searching for universities
// to add to their "watched universities" list from the frontend.

const universitySchema = new mongoose.Schema({
  // Example: university of victoria
  name: {
    type: String,
    required: true,
    unique: true,
    index: true,
  },
  // Example: uvic
  school_code: {
    type: String,
    required: true,
    unique: true,
    index: true,
  },
  // Example: @uvic.ca
  email_suffix: {
    type: String,
    required: true,
  },
  // Example: [-73.97, 40.77]
  location: {
    type: { type: String },
    coordinates: [Number],
  },
});

module.exports = mongoose.model("University", universitySchema);
