const Joi = require("@hapi/joi");

const registerValidation = data => {
    const schema = Joi.object({
        username: Joi.string().min(3).max(30).required().lowercase(),
        email: Joi.string().min(5).max(255).required().email(),
        password: Joi.string().min(6).max(30).required()
    });
    return schema.validate(data);
}

module.exports = registerValidation;