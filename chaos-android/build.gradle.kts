// Top-level build file for the standalone Android app.
tasks.register("clean", Delete::class) {
    delete(layout.buildDirectory)
}

