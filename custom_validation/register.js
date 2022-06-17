const {USERNAME_MAX_LENGTH, USERNAME_MIN_LENGTH, EMAIL_MAX_LENGTH, EMAIL_MIN_LENGTH, PASSWORD_MAX_LENGTH, PASSWORD_MIN_LENGTH} = require("../constants/setup");

function registerValidation(email, username, password) {
    // Email validation
    if (!validateEmail(email)) {
        return "invalid email";
    }
    else if (email.length > EMAIL_MAX_LENGTH) {
        return "email too long";
    } else if (email.length < EMAIL_MIN_LENGTH) {
        return "email too short";
    }
    // Username validation
    if (!username.match("^[a-zA-Z0-9_.-]*$")) {
        return "invalid username format";
    } else if (username.length > USERNAME_MAX_LENGTH) {
        return "username too long";
    } else if (username.length < USERNAME_MIN_LENGTH) {
        return "username too short";
    }
    // Password verification
    if (password.length > PASSWORD_MAX_LENGTH) {
        return "password too long"
    } else if (password.length < PASSWORD_MIN_LENGTH) {
        return "password too short";
    }
    // Passes everything, all good!
    return null;
}

function validateEmail(email) {
    var re = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    return re.test(email);
}

module.exports  = registerValidation;

// min, max, EMAIL VALID, username profanity