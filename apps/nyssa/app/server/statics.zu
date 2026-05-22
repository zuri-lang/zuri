import os
import mime
import hash
import .errors
import ..setup
import log

var static_files_directory = os.join_paths(setup.NYSSA_DIR, setup.STATIC_DIR)
var source_files_directory = os.join_paths(setup.NYSSA_DIR, setup.SOURCES_DIR)

# if static files directory does not exist, create it.
if !os.dir_exists(static_files_directory)
  os.create_dir(static_files_directory)

def static_handler(req, res) {
  var static_path = req.path.replace('/^\\/static\\//', '')
  var reader = file(os.join_paths(static_files_directory, static_path), 'rb')
  
  if reader.exists() {
    res.headers['Content-Type'] = mime.detect_from_name(static_path)
    # cache for 1 year
    res.headers['Cache-Control'] = 'public, max-age=31536000, s-maxage=31536000, immutable'

    var content = reader.read()
    res.headers['Etag'] = 'W/"${hash.md5(content)}"'

    res.write(content)
    content.dispose()
  } else {
    errors.not_found(req, res)
  }
}

def source_handler(req, res) {
  var source_path = req.path.replace('/^\\/source\\//', '')
  var reader = file(os.join_paths(source_files_directory, source_path), 'rb')
  
  if reader.exists() {
    res.headers['Content-Type'] = 'application/octet-stream'
    var content = reader.read()
    res.write(content)
    content.dispose()
  } else {
    errors.not_found(req, res)
  }
}
