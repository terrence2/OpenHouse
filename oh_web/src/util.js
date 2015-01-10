function assert(cond) {
    if (!cond)
        throw "Assertion failure";
}

module.exports = {
    assert: assert
};
