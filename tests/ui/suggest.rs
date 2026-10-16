caiaphas::grammar!(mod number {
    "number = digit, {digit}; digit = ? U+0030..U+0039 ?;"
}, suggest);

fn main() {}
