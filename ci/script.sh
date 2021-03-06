# This script takes care of testing your crate

set -ex

# TODO This is the "test phase", tweak it as you see fit
main() {
    cross build --all --target $TARGET
    cross build --all --target $TARGET --release

    if [ ! -z $DISABLE_TESTS ]; then
        return
    fi

    cross test --all --target $TARGET
    cross test --all --target $TARGET --release

    (cross run -p rrrocket --target $TARGET -- --help)
    (cross run -p rrrocket --target $TARGET --release -- --help)
}

# we don't run the "test phase" when doing deploys
if [ -z $TRAVIS_TAG ]; then
    main
fi
