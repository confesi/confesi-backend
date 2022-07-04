const mongoose = require("mongoose");

const universitySchema = new mongoose.Schema({
  name: {
    type: String,
    required: true,
    unique: true,
    index: true,
  },
  school_code: {
    type: String,
    required: true,
    unique: true,
    index: true,
  },
  email_suffix: {
    type: String,
    required: true,
  },
  location: {
    type: { type: String },
    coordinates: [Number],
  },
});

module.exports = mongoose.model("University", universitySchema);
