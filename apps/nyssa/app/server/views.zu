# FROTEND WEB PAGES
import json
import bcrypt
import os
import log
import .db
import .util
import ..setup

var doc_files = [
  'getting-started.md',
  'creating-projects.md',
  'package-layout.md',
  'managing-dependencies.md',
  'testing.md',
  'test-globals.md',
  'test-assertions.md',
  'install-and-uninstall-actions.md',
  'publishing-packages.md',
  'getting-project-info.md',
  'hosting-a-private-repository.md',
  'publishers-account.md',
  'cleaning-cache-and-logs.md',
  'commands.md',
]

var docs_dir = os.join_paths(setup.NYSSA_DIR, setup.DOCS_DIR)

def error_page(req, res) {
  res.template('404')
}

def home(req, res) {
  res.template('home', {
    publishers: db.get_publishers_count(),
    packages: db.get_packages_count(),
    downloads: db.get_all_download_count(),
    top_packages: db.get_top_packages(),
    latest_packages: db.get_top_packages('created_at DESC'),
    show_login: !res.session.contains('user'),
  })
}

def search(req, res) {
  if !req.queries.contains('q') {
    # redirect to home
    res.redirect('/')
    return
  }

  var query = req.queries.q
  var page = to_number(req.queries.get('page', '1'))
  var sort = req.queries.get('sort', nil) ? req.queries.sort : 'download'
  var real_sort = sort == 'name' ? '${sort} ASC' : '${sort} DESC'

  var result = db.search_package('%${query}%', page, real_sort)
  for pack in result.packages {
    pack.tags = json.decode(pack.tags)
  }

  var pg = 1..(result.pages + 1)
  if result.pages > 11 and page >= 6 {
    var start = page - 5, end = page + 5 + 1
    if end > result.pages end = result.pages
    pg = start..end
  }

  var pagination = []
  for x in pg {
    if x == page pagination.append({page: x, active: true})
    else pagination.append({page: x, active: false})
  }

  res.template('search', {
    query: query,
    result: result,
    pages: pagination,
    sort: sort,
    show_login: !res.session.contains('user'),
  })
}

def view(req, res) {
  var id = req.params.id
  var name = id, version
  if id.match('@') {
    var id_split = id.split('@')
    name = id_split[0]; version = id_split[1]
  }
  var package = db.get_package_for_view(name, version)

  if package {
    package.deps = json.decode(package.deps)
    package.tags = json.decode(package.tags)
    package['versions'] = db.get_package_versions(name)
  } else {
    # redirect to not found
    res.redirect('/404')
    return
  }
  res.template('view', {
    package: package,
    name: req.params.id,
    version: version,
    show_login: !res.session.contains('user'),
  })
}

def authenticate(req, res) {
  if res.session.contains('user') {
    res.redirect('/account')
    return
  }

  if req.body and is_dict(req.body) {
    var name = req.body.get('username', nil),
      password = req.body.get('password', nil)

    if name and password {
      var pub = db.get_publisher(name)

      # authenticate
      if pub and bcrypt.compare(password, pub.password) {
        res.session['user'] = pub
        res.redirect('/account')
        return
      }
    }
  }

  res.redirect('/login?error=1')
}

def login(req, res) {
  if res.session.contains('user') {
    res.redirect('/account')
    return
  }

  res.template('login', {
    show_login: true,
    error: req.queries.get('error', nil)
  })
}

def password_recovery(req, res) {
  if res.session.contains('user') {
    res.redirect('/account')
    return
  }

  res.template('recover', {
    show_login: true,
    error: req.queries.get('error', nil)
  })
}

def recover(req, res) {
  if res.session.contains('user') {
    res.redirect('/account')
    return
  }

  if req.body and is_dict(req.body) {
    var name = req.body.get('username', nil),
      key = req.body.get('key', nil)

    if name and key {
      var pub = db.get_publisher(name, key)

      # authenticate
      if pub {
        res.session['user'] = pub
        res.redirect('/change-password')
        return
      }
    }
  }

  res.redirect('/forgot-password?error=1')
}

def change_password(req, res) {
  if !res.session.contains('user') {
    res.redirect('/login')
    return
  }

  res.template('change_password', {
    user: res.session['user'],
    error: req.queries.get('error', nil)
  })
}

def update_password(req, res) {
  if !res.session.contains('user') {
    res.redirect('/login')
    return
  }

  if req.body and is_dict(req.body) {
    var password = req.body.get('password', nil),
      password_confirm = req.body.get('password-confirm', nil)

    if password and password_confirm {
      if password == password_confirm {
        if !db.update_publisher_password(
          res.session['user'].username, bcrypt.hash(password, 5)
        ) {
          res.redirect('/change-password?error=3')
          return
        }

        res.redirect('/account')
        return
      } else {
        res.redirect('/change-password?error=2')
        return
      }
    }
  }

  res.redirect('/change-password?error=1')
}

def account(req, res) {
  if !res.session.contains('user') {
    res.redirect('/login')
    return
  }

  var page = to_number(req.queries.get('page', '1'))
  var result = db.get_user_packages(res.session['user'].username, page)
  for pack in result.packages {
    pack.tags = json.decode(pack.tags)
    pack.versions = db.get_package_versions(pack.name)
    pack.downloads = db.get_all_download_count(pack.name)
  }

  var pg = 1..(result.pages + 1)
  if result.pages > 11 and page >= 6 {
    var start = page - 5, end = page + 5 + 1
    if end > result.pages end = result.pages
    pg = start..end
  }

  var pagination = []
  for x in pg {
    if x == page pagination.append({page: x, active: true})
    else pagination.append({page: x, active: false})
  }

  res.template('account', {
    result: result,
    pages: pagination,
    show_login: false,
    user: res.session['user'],
    message: res.session.remove('message')
  })
}

def revert(req, res) {
  if !res.session.contains('user') {
    res.redirect('/login')
    return
  }
  
  if req.body and is_dict(req.body) {
    var name = req.body.get('name', nil),
      version = req.body.get('version', nil)

    if name and version {
      var package = db.get_package(name, version)
      if package and package.publisher == res.session['user'].username {
        if db.revert_package(name, package.id) {
          # create flash message
          res.session['message'] = 'Package <strong>${name}</strong> successfully reverted to version <strong>${version}</strong>'
        }
        res.redirect('/account')
        return
      }
    }
  }

  res.redirect('/login')
}

def archive(req, res) {
  if !res.session.contains('user') {
    res.redirect('/login')
    return
  }
  
  if req.params {
    var name = req.params.name
    var package = db.get_package(name)
    if package and package.publisher == res.session['user'].username {
      if db.archive_package(name) {
        # create flash message
        res.session['message'] = 'Package <strong>${name}</strong> successfully archived.'
      }
      res.redirect('/account')
      return
    }
  }

  res.redirect('/login')
}

def logout(req, res) {
  res.clear_session()
  res.redirect('/')
}

def doc(req, res) {
  var uri = req.path.replace('~^/docs/?~', '').trim('/')
  if uri == '' uri = 'getting-started'
  
  var doc_file
  if !doc_files.contains('${uri}.md') or 
    !os.dir_exists(docs_dir) or
    !file(doc_file = os.join_paths(docs_dir, '${uri}.md')).exists() {
    res.redirect('/404')
    return
  }

  res.template('doc', {
    uri: uri,
    content: file(doc_file).read(),
    doc_files: doc_files,
    show_login: !res.session.contains('user'),
  })
}
