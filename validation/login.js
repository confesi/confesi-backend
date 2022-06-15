const Joi = require("@hapi/joi");

const loginValidation = data => {
    const schema = Joi.object({
        usernameOrEmail: Joi.string().min(3).max(255).required(),
        password: Joi.string().min(6).max(30).required()
    });
    return schema.validate(data);
}

module.exports = loginValidation;