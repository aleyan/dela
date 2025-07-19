cmake_minimum_required(VERSION 3.10)
project(DelaTest)

# Basic custom targets
add_custom_target(build-all COMMENT "Build all components")
add_custom_target(test-all COMMENT "Run all tests")
add_custom_target(clean-all COMMENT "Clean all build artifacts")

# More complex targets
add_custom_target(
    deploy
    COMMAND echo "Deploying application..."
    COMMENT "Deploy the application to production"
)

add_custom_target(install COMMENT "Install dependencies")
add_custom_target(docs COMMENT "Generate documentation") 