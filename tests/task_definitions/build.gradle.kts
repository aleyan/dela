plugins {
    id("java")
    id("application")
}

repositories {
    mavenCentral()
}

dependencies {
    implementation("org.slf4j:slf4j-api:1.7.30")
    testImplementation("junit:junit:4.13.2")
}

tasks.register<DefaultTask>("kotlinGradleTest") {
    description = "A test task for Kotlin Gradle DSL"
    doLast {
        println("Running Kotlin Gradle test task")
    }
}

tasks.register<DefaultTask>("kotlinGradleBuild") {
    description = "A build task for Kotlin Gradle DSL"
    doLast {
        println("Building with Kotlin Gradle")
    }
}

application {
    mainClass.set("com.example.Main")
} 