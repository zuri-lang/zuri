import os
import ..setup

def parse(parser) {
  parser.add_option(
    'version', 
    'Show Nyssa version', 
    {
      short_name: 'v'
    }
  )
}

def get(value) {
  echo 'Nyssa ${setup.NYSSA_VERSION}'
  echo os.exec('${os.args[0]} -v')
}
