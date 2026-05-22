import date
import .config

class Package {
  var publisher
  var name
  var version
  var source
  var readme
  var config
  var downloads = 0
  var active = true
  var created_at
  var deleted_at

  Package(publisher, name, version, source, config, readme, created_at) {
    self.publisher = publisher
    self.name = name
    self.version = version
    self.source = source
    self.config = config
    self.readme = readme
    self.created_at = created_at ? created_at : date()
  }

  static from_dict(dict) {
    var package = Package(dict.publisher, dict.name, dict.version, dict.source, dict.config, dict.readme, dict.get('created_at', date().format('c')))
    package.downloads = dict.get('downloads', 0)
    package.active = dict.get('active', true)
    package.deleted_at = dict.get('deleted_at', nil)
    return package
  }

  @to_json() {
    return {
      name: self.name,
      publisher: self.publisher,
      version: self.version,
      source: self.source,
      config: self.config,
      readme: self.readme,
      downloads: self.downloads,
      active: self.active,
      created_at: self.created_at,
      deleted_at: self.deleted_at,
    }
  }
}

def package(publisher, name, version, config, readme, source) {
  return Package.from_dict({
    name: name,
    publisher: publisher,
    version: version,
    config: config,
    source: source,
    readme: readme,
  })
}
