import http
import log
import .router { * }

def server(host, port) {
  
  var server = http.server(port, host)
  server.on_receive(router)
  server.on_error(@(err, _) {
    log.error('Error: ${err.message}\nTrace:\n${err.stacktrace}')
  })

  var host_name = host == '0.0.0.0' ? 'localhost' : host
  log.info('Nyssa repository server started.') 
  log.info('Repository URL: http://${host_name}:${port}')
  server.listen()
}
