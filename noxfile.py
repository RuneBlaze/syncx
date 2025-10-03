import nox

PYTHONS = ("3.9", "3.10", "3.11", "3.12", "3.13")
DEV_DEPS = (
    "maturin>=1.4,<2.0",
    "pytest>=8.3.5",
    "pytest-timeout>=2.4.0",
)

nox.options.default_venv_backend = "uv"
nox.options.sessions = ("tests", "tests-free-threaded")

def _build_extension(session: nox.Session, *, free_threaded: bool) -> None:
    args = ["develop", "--release"]
    env = {}
    if free_threaded:
        args.append("--no-default-features")
        env["PYTHON_GIL"] = "0"
    session.run("maturin", *args, env=env)


def _run_pytest(session: nox.Session) -> None:
    args = session.posargs or ["-q", "tests"]
    session.run("pytest", *args)


@nox.session(python=PYTHONS)
def tests(session: nox.Session) -> None:
    session.install(*DEV_DEPS)
    _build_extension(session, free_threaded=False)
    _run_pytest(session)


@nox.session(name="tests-free-threaded", python="3.13t")
def tests_free_threaded(session: nox.Session) -> None:
    session.install(*DEV_DEPS)
    _build_extension(session, free_threaded=True)
    _run_pytest(session)
