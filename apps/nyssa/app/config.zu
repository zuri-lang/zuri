import .setup

class Config {
  var name
  var version
  var description
  var homepage
  var tags = []
  var author
  var license
  var deps = {}
  var cvs_deps = []
  var post_install
  var pre_uninstall
  var cli

  # list of default sources
  var sources = [
    setup.DEFAULT_REPOSITORY
  ]

  static from_dict(dict) {
    var c = Config()

    c.name = dict.get('name', nil)
    c.version = dict.get('version', '1.0.0')
    c.description = dict.get('description', nil)
    c.homepage = dict.get('homepage', nil)
    c.tags = dict.get('tags', [])
    c.author = dict.get('author', nil)
    c.license = dict.get('license', 'ISC')
    c.deps = dict.get('deps', {})
    c.cvs_deps = dict.get('cvs_deps', {})
    c.post_install = dict.get('post_install', nil)
    c.pre_uninstall = dict.get('pre_uninstall', nil)
    c.cli = dict.get('cli', nil)

    return c
  }

  @to_json() {
    return {
      name: self.name,
      version: self.version,
      description: self.description,
      homepage: self.homepage,
      tags: self.tags,
      author: self.author,
      license: self.license,
      sources: self.sources,
      deps: self.deps,
      cvs_deps: self.cvs_deps,
      post_install: self.post_install,
      pre_uninstall: self.pre_uninstall,
      cli: self.cli,
    }.compact()
  }
}
