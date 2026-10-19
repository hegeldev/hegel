import hegel

project = "Hegel for Python"
author = "Hegel developers"
copyright = "the Hegel developers"
release = hegel.__version__
version = release

extensions = [
    "myst_parser",
    "sphinx.ext.autodoc",
    "sphinx.ext.intersphinx",
]

exclude_patterns = ["_build"]

myst_enable_extensions = ["colon_fence", "deflist"]
myst_heading_anchors = 3

autodoc_member_order = "bysource"
autodoc_typehints = "signature"
autodoc_preserve_defaults = True

intersphinx_mapping = {
    "python": ("https://docs.python.org/3", None),
    "pytest": ("https://docs.pytest.org/en/stable", None),
}

html_theme = "furo"
html_title = f"Hegel for Python {release}"
html_theme_options = {
    "source_repository": "https://github.com/hegeldev/hegel",
    "source_branch": "main",
    "source_directory": "frontends/python/docs/",
}
