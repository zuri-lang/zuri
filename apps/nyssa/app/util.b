import os
import template
import log
import .setup

def setup_cli(name, base_path, cli_path) {
  var is_windows = os.platform == 'windows'
  var path = os.join_paths(base_path, '../../', name) + (is_windows ? '.cmd' : '')

  var cmd
  if !is_windows {
    cmd = '#!/bin/bash\n' +
      '\n' +
      'if [[ ! $(command -v zuri) ]]; then\n' +
      '  echo "Zuri is not installed or not in path."\n' +
      '  exit 1\n' +
      'fi\n' +
      '\n'+ 
      'ZURI_EXE=`which zuri`\n' +
      '\n' +
      'exec "$ZURI_EXE" "${cli_path}" "$@"\n'
  } else {

    cmd = '@ECHO OFF\r\n' +
      'SETLOCAL\r\n' +
      '\r\n' +
      '@REM Find Zuri executable\r\n' +
      'for %%i in (zuri.exe) do SET "ZURI_EXE=%%~$PATH:i"\r\n' +
      '\r\n' +
      'if defined ZURI_EXE (\r\n' +
      '    GOTO :RUN\r\n' +
      ') else (\r\n' +
      '    GOTO :ERROR\r\n' +
      ')\r\n' +
      '\r\n' +
      ':ERROR\r\n' +
      'SET msgboxTitle=${name}\r\n' +
      'SET msgboxBody=Zuri is not installed or not in path.\r\n' +
      'SET tmpmsgbox=%temp%\~tmpmsgbox.vbs\r\n' +
      'IF EXIST "%tmpmsgbox%" DEL /F /Q "%tmpmsgbox%"\r\n' +
      'ECHO msgbox "%msgboxBody%",0,"%msgboxTitle%">"%tmpmsgbox%"\r\n' +
      'WSCRIPT "%tmpmsgbox%"\r\n' +
      'EXIT /B 1\r\n' +
      '\r\n' +
      ':RUN\r\n' +
      '\r\n' +
      '"%ZURI_EXE%" "${cli_path}" %*\r\n' +
      'EXIT /B 0\r\n'
  }


  file(path, 'w').write(cmd)
  if !is_windows {
    os.chmod(path, 0c744) # u+x
  }
}

def remove_cli(name, base_path) {
  var is_windows = os.platform == 'windows'
  var path = os.join_paths(base_path, '../../', name) + (is_windows ? '.cmd' : '')
  var fh = file(path)
  if fh.exists()
    fh.delete()
}

def setup_template() {
  var tpl = template()
  tpl.set_root(os.join_paths(setup.NYSSA_DIR, setup.TEMPLATES_DIR))
  return tpl.render
}
