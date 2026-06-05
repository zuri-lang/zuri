#ifdef _MSC_VER
# pragma warning (disable : 5105)
#endif

#include "module.h"
#include "pathinfo.h"

#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>

#ifdef HAVE_UNISTD_H
# include <unistd.h>
#else
# include "zunistd.h"
#endif /* HAVE_UNISTD_H */

#ifdef _WIN32
# define _WINSOCK_DEPRECATED_NO_WARNINGS 1
# include <sdkddkver.h>
# include <ws2tcpip.h>
# include <winsock2.h>
# include <zunistd.h>

# define sleep			_sleep
# ifndef strcasecmp
#   define strcasecmp		strcmpi
# endif
# define ioctl ioctlsocket
# ifndef STDIN_FILENO
#   define STDIN_FILENO _fileno(stdin)
# endif
#else
# include <sys/socket.h>
# include <arpa/inet.h>
# include <netdb.h> //hostent
# include <sys/ioctl.h>
# include <sys/ioctl.h>
# include <sys/socket.h>
# include <poll.h>
# include <netinet/tcp.h>
# define closesocket close
#endif

#define BIGSIZ 8192    /* big buffers */
#define SMALLSIZ 256    /* small buffers, hostnames, etc. */

// Cross-platform socket error helpers
static inline int last_sock_error(void) {
#ifdef _WIN32
  return WSAGetLastError();
#else
  return errno;
#endif
}

static inline int map_sock_err_to_errno(int se) {
#ifdef _WIN32
  switch (se) {
    case WSAEWOULDBLOCK: return EWOULDBLOCK;
    case WSAEINPROGRESS: return EINPROGRESS;
    case WSAEINTR:       return EINTR;
    case WSAETIMEDOUT:   return ETIMEDOUT;
    case WSAECONNREFUSED:return ECONNREFUSED;
    case WSAECONNRESET:  return ECONNRESET;
    case WSAEADDRINUSE:  return EADDRINUSE;
    case WSAEADDRNOTAVAIL:return EADDRNOTAVAIL;
    case WSAENETUNREACH: return ENETUNREACH;
    case WSAEHOSTUNREACH:return EHOSTUNREACH;
    default:             return EIO;
  }
#else
  (void)se; return errno;
#endif
}

static inline int is_would_block(int se) {
#ifdef _WIN32
  return se == WSAEWOULDBLOCK;
#else
  return se == EAGAIN || se == EWOULDBLOCK;
#endif
}

static inline int is_in_progress(int se) {
#ifdef _WIN32
  return se == WSAEINPROGRESS || se == WSAEWOULDBLOCK;
#else
  return se == EINPROGRESS;
#endif
}

static inline int is_interrupted(int se) {
#ifdef _WIN32
  return se == WSAEINTR;
#else
  return se == EINTR;
#endif
}

static inline int is_timed_out(int se) {
#ifdef _WIN32
  return se == WSAETIMEDOUT;
#else
  return se == ETIMEDOUT;
#endif
}

#ifdef _WIN32
static const char *strerror_socket(int err) {
  switch (err) {
    case WSAEWOULDBLOCK:   return "Resource temporarily unavailable (would block)";
    case WSAEINPROGRESS:   return "Operation now in progress";
    case WSAEALREADY:      return "Operation already in progress";
    case WSAENOTSOCK:      return "Socket operation on non-socket";
    case WSAEDESTADDRREQ:  return "Destination address required";
    case WSAEMSGSIZE:      return "Message too long";
    case WSAEPROTOTYPE:    return "Protocol wrong type for socket";
    case WSAENOPROTOOPT:   return "Protocol not available";
    case WSAEPROTONOSUPPORT: return "Protocol not supported";
    case WSAESOCKTNOSUPPORT: return "Socket type not supported";
    case WSAEOPNOTSUPP:    return "Operation not supported";
    case WSAEPFNOSUPPORT:  return "Protocol family not supported";
    case WSAEAFNOSUPPORT:  return "Address family not supported by protocol family";
    case WSAEADDRINUSE:    return "Address already in use";
    case WSAEADDRNOTAVAIL: return "Cannot assign requested address";
    case WSAENETDOWN:      return "Network is down";
    case WSAENETUNREACH:   return "Network is unreachable";
    case WSAENETRESET:     return "Network dropped connection on reset";
    case WSAECONNABORTED:  return "Software caused connection abort";
    case WSAECONNRESET:    return "Connection reset by peer";
    case WSAENOBUFS:       return "No buffer space available";
    case WSAEISCONN:       return "Socket is already connected";
    case WSAENOTCONN:      return "Socket is not connected";
    case WSAESHUTDOWN:     return "Cannot send after socket shutdown";
    case WSAETOOMANYREFS:  return "Too many references: cannot splice";
    case WSAETIMEDOUT:     return "Connection timed out";
    case WSAECONNREFUSED:  return "Connection refused";
    case WSAELOOP:         return "Too many levels of symbolic links";
    case WSAENAMETOOLONG:  return "File name too long";
    case WSAEHOSTDOWN:     return "Host is down";
    case WSAEHOSTUNREACH:  return "No route to host";
    case WSAENOTEMPTY:     return "Directory not empty";
    case WSAEPROCLIM:      return "Too many processes";
    case WSAEUSERS:        return "Too many users";
    case WSAEDQUOT:        return "Disc quota exceeded";
    case WSAESTALE:        return "Stale NFS file handle";
    case WSAEREMOTE:       return "Too many levels of remote in path";
    case WSAEDISCON:       return "Graceful shutdown in progress";
#ifdef HOST_NOT_FOUND
    case HOST_NOT_FOUND:   return "Host not found";
#endif
#ifdef TRY_AGAIN
    case TRY_AGAIN:        return "Non-authoritative host not found";
#endif
#ifdef NO_RECOVERY
    case NO_RECOVERY:      return "Non-recoverable keyboard/system error";
#endif
#ifdef NO_DATA
    case NO_DATA:          return "Valid name, no data record of requested type";
#endif
    case EWOULDBLOCK:      return "Operation would block";
    case EINPROGRESS:      return "Operation now in progress";
    case EINTR:            return "Interrupted system call";
    case ETIMEDOUT:        return "Connection timed out";
    case ECONNREFUSED:     return "Connection refused";
    case ECONNRESET:       return "Connection reset by peer";
    case EADDRINUSE:       return "Address already in use";
    case EADDRNOTAVAIL:    return "Cannot assign requested address";
    case ENETUNREACH:      return "Network is unreachable";
    case EHOSTUNREACH:     return "No route to host";
    case EIO:              return "Input/output error";
    default: {
      static __thread char msgbuf[256];
      DWORD len = FormatMessageA(
          FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
          NULL,
          err,
          MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT),
          msgbuf,
          sizeof(msgbuf) - 1,
          NULL);
      if (len > 0) {
        while (len > 0 && (msgbuf[len - 1] == '\r' || msgbuf[len - 1] == '\n' || msgbuf[len - 1] == ' ' || msgbuf[len - 1] == '.')) {
          msgbuf[--len] = '\0';
        }
        return msgbuf;
      }
      return "Unknown socket error";
    }
  }
}
#endif

static void socket_configure_fd(int sock) {
#ifndef _WIN32
# ifdef SO_NOSIGPIPE
  int set = 1;
  (void)setsockopt(sock, SOL_SOCKET, SO_NOSIGPIPE, &set, sizeof(int));
# endif
# ifndef SOCK_CLOEXEC
  int flags = fcntl(sock, F_GETFD);
  if (flags != -1) {
    (void)fcntl(sock, F_SETFD, flags | FD_CLOEXEC);
  }
# else
  /* SOCK_CLOEXEC applied at socket()/accept4() time; nothing to do */
# endif
#else
  (void)sock;
#endif
}

DECLARE_MODULE_METHOD(socket__error) {
  ENFORCE_ARG_COUNT(error, 1);
  ENFORCE_ARG_TYPE(error, 0, IS_NUMBER);

  if (AS_NUMBER(args[0]) == -1) {
    int se = last_sock_error();
    if (!is_in_progress(se) && !is_would_block(se)) {
#ifdef _WIN32
      errno = map_sock_err_to_errno(se);
      const char *msg = strerror_socket(se);
#else
      const char *msg = strerror(errno);
#endif
      RETURN_STRING(msg);
    }
  }
  RETURN_NIL;
}

DECLARE_MODULE_METHOD(socket__create) {
  ENFORCE_ARG_COUNT(create, 3);
  ENFORCE_ARG_TYPE(create, 0, IS_NUMBER); // family
  ENFORCE_ARG_TYPE(create, 1, IS_NUMBER); // type
  ENFORCE_ARG_TYPE(create, 2, IS_NUMBER); // protocol

  int family = (int) AS_NUMBER(args[0]);
  int type = (int) AS_NUMBER(args[1]);

#ifdef SOCK_CLOEXEC
  type |= SOCK_CLOEXEC;
#endif

  int protocol = (int) AS_NUMBER(args[2]);

  int sock;
  if ((sock = socket(family, type, protocol)) < 0) {
    RETURN_NUMBER(-1);
  }

  socket_configure_fd(sock);

  RETURN_NUMBER(sock);
}

DECLARE_MODULE_METHOD(socket__connect) {
  ENFORCE_ARG_COUNT(connect, 6);
  ENFORCE_ARG_TYPE(connect, 0, IS_NUMBER); // the socket id
  ENFORCE_ARG_TYPE(connect, 1, IS_STRING); // the address
  ENFORCE_ARG_TYPE(connect, 2, IS_NUMBER); // the port
  ENFORCE_ARG_TYPE(connect, 3, IS_NUMBER); // the family
  ENFORCE_ARG_TYPE(connect, 4, IS_NUMBER); // timeout (ms)
  ENFORCE_ARG_TYPE(connect, 5, IS_BOOL);   // is_blocking

  int sock = AS_NUMBER(args[0]);
  char *address = AS_C_STRING(args[1]);
  int port = AS_NUMBER(args[2]);
  int family = AS_NUMBER(args[3]);
  int time_out = AS_NUMBER(args[4]);
  bool is_blocking = AS_BOOL(args[5]);

  // Resolve address using getaddrinfo for IPv4/IPv6 support
  char port_str[16];
  snprintf(port_str, sizeof(port_str), "%d", port);

  struct addrinfo hints; memset(&hints, 0, sizeof(hints));
  hints.ai_socktype = 0; // allow any; we already created socket
  hints.ai_family = family; // AF_INET, AF_INET6, or AF_UNSPEC

  struct addrinfo *res = NULL, *rp = NULL;
  int gai_rc = getaddrinfo(address, port_str, &hints, &res);
  if (gai_rc != 0 || res == NULL) {
    errno = EADDRNOTAVAIL;
    RETURN_NUMBER(-1);
  }

  // Setup for nonblocking connect if a timeout is requested
  bool toggled_nonblock = false;
#ifndef _WIN32
  int old_flags = 0;
  if (time_out > 0) {
    old_flags = fcntl(sock, F_GETFL);
    if (old_flags != -1 && (old_flags & O_NONBLOCK) == 0) {
      if (fcntl(sock, F_SETFL, old_flags | O_NONBLOCK) == 0) toggled_nonblock = true;
    } else if (old_flags != -1) {
      toggled_nonblock = true; // already nonblocking
    }
  }
#else
  unsigned long nbarg = 1;
  if (time_out > 0) {
    // Querying blocking mode portably is tricky on Windows; just set nonblocking
    // and restore to blocking later if requested
    if (ioctl(sock, FIONBIO, &nbarg) == 0) toggled_nonblock = true;
  }
#endif

  int last_errno = 0;
  int result = -1;

  for (rp = res; rp != NULL; rp = rp->ai_next) {
    int rc;
#if !defined(_WIN32) && defined(EINTR)
    for (;;) {
      rc = connect(sock, rp->ai_addr, (socklen_t)rp->ai_addrlen);
      if (rc >= 0 || errno != EINTR) break;
    }
#else
    rc = connect(sock, rp->ai_addr, (socklen_t)rp->ai_addrlen);
#endif
    if (rc == 0) { result = 0; break; }

    // If using timeout, INPROGRESS/WOULDBLOCK are acceptable; wait for writability
    {
      int se = last_sock_error();
      if (is_in_progress(se) || is_would_block(se)) {
        if (time_out > 0) {
#ifdef _WIN32
          WSAPOLLFD pfd = { (SOCKET)sock, POLLOUT, 0 };
          int sel = WSAPoll(&pfd, 1, time_out);
#else
          struct pollfd pfd = { sock, POLLOUT, 0 };
          int sel = poll(&pfd, 1, time_out);
#endif
          if (sel > 0) {
            int so_error = 0; socklen_t len = (socklen_t)sizeof(so_error);
#ifndef _WIN32
            (void)getsockopt(sock, SOL_SOCKET, SO_ERROR, &so_error, &len);
#else
            (void)getsockopt(sock, SOL_SOCKET, SO_ERROR, (char*)&so_error, &len);
#endif
            if (so_error == 0) { result = 0; break; }
#ifdef _WIN32
            WSASetLastError(so_error);
            last_errno = map_sock_err_to_errno(so_error);
#else
            last_errno = so_error;
#endif
          } else if (sel == 0) {
#ifdef _WIN32
            WSASetLastError(WSAETIMEDOUT);
#endif
            last_errno = ETIMEDOUT;
          } else {
#ifdef _WIN32
            last_errno = map_sock_err_to_errno(last_sock_error());
#else
            last_errno = errno;
#endif
          }
        } else {
#ifdef _WIN32
          last_errno = map_sock_err_to_errno(se);
#else
          last_errno = se;
#endif
        }
      } else {
#ifdef _WIN32
        last_errno = map_sock_err_to_errno(se);
#else
        last_errno = se;
#endif
      }
    }
  }

  freeaddrinfo(res);

  // Restore blocking mode if needed
#ifndef _WIN32
  if (toggled_nonblock && is_blocking && old_flags != -1) {
    (void)fcntl(sock, F_SETFL, old_flags);
  }
#else
  if (toggled_nonblock && is_blocking) {
    unsigned long zero = 0; (void)ioctl(sock, FIONBIO, &zero);
  }
#endif

  if (result == 0) RETURN_NUMBER(0);
  if (last_errno != 0) errno = last_errno;
  RETURN_NUMBER(-1);
}

DECLARE_MODULE_METHOD(socket__bind) {
  ENFORCE_ARG_COUNT(bind, 4);
  ENFORCE_ARG_TYPE(bind, 0, IS_NUMBER); // the socket id
  ENFORCE_ARG_TYPE(bind, 1, IS_STRING); // the address (or empty string/nil for wildcard)
  ENFORCE_ARG_TYPE(bind, 2, IS_NUMBER); // the port
  ENFORCE_ARG_TYPE(bind, 3, IS_NUMBER); // the family

  int sock = AS_NUMBER(args[0]);
  char *address = AS_C_STRING(args[1]);
  int port = AS_NUMBER(args[2]);
  int family = AS_NUMBER(args[3]);

  // Use getaddrinfo with sockaddr_storage for IPv4/IPv6; allow AI_PASSIVE for wildcard
  char port_str[16];
  snprintf(port_str, sizeof(port_str), "%d", port);

  struct addrinfo hints; memset(&hints, 0, sizeof(hints));
  hints.ai_family = family; // AF_INET/AF_INET6/AF_UNSPEC
  hints.ai_socktype = 0; // any
  hints.ai_flags = 0;
  if (address == NULL || address[0] == '\0' || strcmp(address, "*") == 0) {
    hints.ai_flags |= AI_PASSIVE;
    address = NULL; // let system choose ANY address
  }

  struct addrinfo *res = NULL, *rp = NULL;
  int gai_rc = getaddrinfo(address, port_str, &hints, &res);
  if (gai_rc != 0 || res == NULL) {
    RETURN_VALUE_ERROR("address not valid or unsupported");
  }

  int rc = -1;
  for (rp = res; rp != NULL; rp = rp->ai_next) {
    rc = bind(sock, rp->ai_addr, (socklen_t)rp->ai_addrlen);
    if (rc == 0) break;
  }
  freeaddrinfo(res);

  RETURN_NUMBER(rc);
}

DECLARE_MODULE_METHOD(socket__listen) {
  ENFORCE_ARG_COUNT(listen, 2);
  ENFORCE_ARG_TYPE(listen, 0, IS_NUMBER); // the socket id
  ENFORCE_ARG_TYPE(listen, 1, IS_NUMBER); // backlog

  int sock = AS_NUMBER(args[0]);
  int backlog = AS_NUMBER(args[1]);

  RETURN_NUMBER(listen(sock, backlog));
}

DECLARE_MODULE_METHOD(socket__accept) {
  ENFORCE_ARG_COUNT(accept, 1);
  ENFORCE_ARG_TYPE(accept, 0, IS_NUMBER); // the socket id

  int sock = AS_NUMBER(args[0]);

  struct sockaddr_storage ss; memset(&ss, 0, sizeof(ss));
  socklen_t client_length = (socklen_t)sizeof(ss);

  int new_sock;
#if defined(__linux__) && defined(SOCK_CLOEXEC)
  new_sock = accept4(sock, (struct sockaddr *)&ss, &client_length, SOCK_CLOEXEC);
#else
  new_sock = accept(sock, (struct sockaddr *)&ss, &client_length);
#endif

  if (new_sock < 0) {
    RETURN_ERROR("client accept failed");
  }

  socket_configure_fd(new_sock);

  z_obj_list *response = (z_obj_list *)GC(new_list(vm));

  char *ip = NULL;
  int port = 0;
  if (ss.ss_family == AF_INET) {
    struct sockaddr_in *sa = (struct sockaddr_in *)&ss;
    ip = ALLOCATE(char, INET_ADDRSTRLEN);
    if (inet_ntop(AF_INET, &sa->sin_addr, ip, INET_ADDRSTRLEN) == NULL) {
      FREE_ARRAY(char, ip, INET_ADDRSTRLEN); ip = NULL;
    } else {
      port = (int)ntohs(sa->sin_port);
    }
  }
#ifdef AF_INET6
  else if (ss.ss_family == AF_INET6) {
    struct sockaddr_in6 *sa6 = (struct sockaddr_in6 *)&ss;
    ip = ALLOCATE(char, INET6_ADDRSTRLEN);
    if (inet_ntop(AF_INET6, &sa6->sin6_addr, ip, INET6_ADDRSTRLEN) == NULL) {
      FREE_ARRAY(char, ip, INET6_ADDRSTRLEN); ip = NULL;
    } else {
      port = (int)ntohs(sa6->sin6_port);
    }
  }
#endif

  write_list(vm, response, NUMBER_VAL(new_sock));
  if (ip != NULL) {
    write_list(vm, response, GC_TT_STRING(ip));
  } else {
    write_list(vm, response, GC_L_STRING("", 0));
  }
  write_list(vm, response, NUMBER_VAL(port));

  RETURN_OBJ(response);
}

DECLARE_MODULE_METHOD(socket__send) {
  ENFORCE_ARG_COUNT(send, 3);
  ENFORCE_ARG_TYPE(send, 0, IS_NUMBER); // the socket id
  ENFORCE_ARG_TYPE(send, 2, IS_NUMBER); // flags

  int sock = AS_NUMBER(args[0]);
  z_value data = args[1];
  int flags = AS_NUMBER(args[2]);

  char *content = NULL;
  int length = 0;
  bool file_content = false; // tracks whether we must free content ourselves

  if (IS_STRING(data)) {
    content = AS_STRING(data)->chars;
    length = AS_STRING(data)->length;
  } else if (IS_BYTES(data)) {
    content = (char *)AS_BYTES(data)->bytes.bytes;
    length = AS_BYTES(data)->bytes.count;
  } else if (IS_FILE(data)) {
    char *path = realpath(AS_FILE(data)->path->chars, NULL);
    if (path == NULL) {
      errno = ENOENT;
      RETURN_NUMBER(-1);
    }

    /* Obtain the file size before reading so we don't rely on strlen */
    FILE *fp = fopen(path, "rb");
    free(path);
    if (fp == NULL) {
      RETURN_NUMBER(-1);
    }
    if (fseek(fp, 0, SEEK_END) != 0) {
      fclose(fp);
      RETURN_NUMBER(-1);
    }
    long file_size = ftell(fp);
    rewind(fp);
    if (file_size < 0) {
      fclose(fp);
      RETURN_NUMBER(-1);
    }

    content = (char *)malloc((size_t)file_size);
    if (content == NULL) {
      fclose(fp);
      RETURN_NUMBER(-1);
    }
    length = (int)fread(content, 1, (size_t)file_size, fp);
    fclose(fp);
    file_content = true; // we own this buffer; free after send
  } else {
    z_obj_string *data_str = value_to_string(vm, data);
    content = data_str->chars;
    length = data_str->length;
  }

#ifndef _WIN32
#ifdef MSG_NOSIGNAL
  flags |= MSG_NOSIGNAL;
#endif
#endif

  int processed = 0;

  // Determine send timeout (if any) using SO_SNDTIMEO
  struct timeval send_timeout_base; int optlen = sizeof(send_timeout_base);
  send_timeout_base.tv_sec = 0; send_timeout_base.tv_usec = 0;
#ifndef _WIN32
  (void)getsockopt(sock, SOL_SOCKET, SO_SNDTIMEO, &send_timeout_base, (socklen_t*)&optlen);
#else
  (void)getsockopt(sock, SOL_SOCKET, SO_SNDTIMEO, (char*)&send_timeout_base, (socklen_t*)&optlen);
#endif

  bool has_timeout = (send_timeout_base.tv_sec > 0 || send_timeout_base.tv_usec > 0);
  int timeout_ms = (int)(send_timeout_base.tv_sec * 1000 +
                         send_timeout_base.tv_usec / 1000);
  const int SEND_CHUNK = 65536;

  while (processed < length) {
    int write_size = (length - processed) < SEND_CHUNK
                         ? (length - processed) : SEND_CHUNK;
    int rc = (int)send(sock, content + processed, (size_t)write_size, flags);
    if (rc > 0) {
      processed += rc;
      continue;
    }
    if (rc == 0) break; // peer closed

    // rc < 0
    {
      int se = last_sock_error();
      if (is_interrupted(se)) {
        continue; // interrupted by signal, retry immediately
      }
      if (is_would_block(se)) {
        if (has_timeout) {
#ifdef _WIN32
          WSAPOLLFD pfd = { (SOCKET)sock, POLLOUT, 0 };
          int sel = WSAPoll(&pfd, 1, timeout_ms);
#else
          struct pollfd pfd = { sock, POLLOUT, 0 };
          int sel = poll(&pfd, 1, timeout_ms);
#endif
          if (sel > 0) continue;
          if (sel == 0) {
#ifdef _WIN32
            WSASetLastError(WSAETIMEDOUT);
#else
            errno = ETIMEDOUT;
#endif
            break;
          }
          // poll/WSAPoll error: fall through to return error
        } else {
          RETURN_NUMBER(-1);
        }
      }
      // Any other error: stop
      if (file_content) free(content);
      RETURN_NUMBER(-1);
    }
  }

  if (file_content) free(content);
  RETURN_NUMBER(processed);
}

DECLARE_MODULE_METHOD(socket__recv) {
  ENFORCE_ARG_COUNT(recv, 3);
  ENFORCE_ARG_TYPE(recv, 0, IS_NUMBER);
  ENFORCE_ARG_TYPE(recv, 1, IS_NUMBER);
  ENFORCE_ARG_TYPE(recv, 2, IS_NUMBER);

  int sock   = AS_NUMBER(args[0]);
  int length = AS_NUMBER(args[1]);
  int flags  = AS_NUMBER(args[2]);

  // Determine receive timeout in milliseconds
  int timeout_ms;
#ifndef _WIN32
  struct timeval tv;
  int option_length = sizeof(tv);
  int rc = getsockopt(sock, SOL_SOCKET, SO_RCVTIMEO, &tv, (socklen_t *)&option_length);
  if (rc != 0 || (int)sizeof(tv) != option_length ||
      (tv.tv_sec == 0 && tv.tv_usec == 0)) {
    timeout_ms = 500; // default 500 ms
  } else {
    timeout_ms = (int)(tv.tv_sec * 1000 + tv.tv_usec / 1000);
  }
#else
  DWORD tv_ms = 0;
  int option_length = sizeof(tv_ms);
  int rc = getsockopt(sock, SOL_SOCKET, SO_RCVTIMEO, (char *)&tv_ms,
                      (socklen_t *)&option_length);
  if (rc != 0 || option_length != sizeof(tv_ms) || tv_ms == 0) {
    timeout_ms = 500; // default 500 ms
  } else {
    timeout_ms = (int)tv_ms;
  }
#endif

#ifdef _WIN32
  WSAPOLLFD pfd = { (SOCKET)sock, POLLIN, 0 };
  int status = WSAPoll(&pfd, 1, timeout_ms);
#else
  struct pollfd pfd = { sock, POLLIN, 0 };
  int status = poll(&pfd, 1, timeout_ms);
#endif

  if (status > 0 && (pfd.revents & POLLIN)) {

    int content_length = 0;
#ifndef _WIN32
    (void)ioctl(sock, FIONREAD, &content_length);
#else
    {
      u_long fionread_val = 0;
      (void)ioctl(sock, FIONREAD, &fionread_val);
      content_length = (int)fionread_val;
    }
#endif

    if (content_length > 0) {
      if (length != -1 && length < content_length)
        content_length = length;

      char *response = ALLOCATE(char, content_length + 1);

      int total_length = 0;
      while (total_length < content_length) {
        int chunk = (int)recv(sock, response + total_length,
                              (size_t)(content_length - total_length), flags);
        if (chunk > 0) {
          total_length += chunk;
          continue;
        }
        if (chunk == 0) break;
        {
          int se = last_sock_error();
          if (is_interrupted(se)) continue;
          if (is_would_block(se)) {
#ifdef _WIN32
            WSAPOLLFD rpfd = { (SOCKET)sock, POLLIN, 0 };
            int sel2 = WSAPoll(&rpfd, 1, timeout_ms);
#else
            struct pollfd rpfd = { sock, POLLIN, 0 };
            int sel2 = poll(&rpfd, 1, timeout_ms);
#endif
            if (sel2 > 0 && (rpfd.revents & POLLIN)) continue;
            if (sel2 == 0) {
#ifdef _WIN32
              WSASetLastError(WSAETIMEDOUT);
#else
              errno = ETIMEDOUT;
#endif
              break;
            }
          }
        }
        break;
      }
      response[total_length] = '\0';
      RETURN_T_STRING(response, total_length);
    }
  } else if (status == 0) {
#ifdef _WIN32
    WSASetLastError(WSAETIMEDOUT);
#else
    errno = ETIMEDOUT;
#endif
    RETURN_NUMBER(-1);
  }

  RETURN_NIL;
}

DECLARE_MODULE_METHOD(socket__read) {
  ENFORCE_ARG_COUNT(read, 3);
  ENFORCE_ARG_TYPE(read, 0, IS_NUMBER); // the socket id
  ENFORCE_ARG_TYPE(read, 1, IS_NUMBER); // length to read
  ENFORCE_ARG_TYPE(read, 2, IS_NUMBER); // flags

  int sock = AS_NUMBER(args[0]);
  int length = AS_NUMBER(args[1]);
  int flags = AS_NUMBER(args[2]);

  if (length <= 0) length = 1024;

  int total_length = 0;
  char *response = ALLOCATE(char, length + 1);

  char buf[4096];
  int bytes_received;

  while (total_length < length &&
         (bytes_received = (int)recv(sock, buf,
             (size_t)((length - total_length) < 4096
                          ? (length - total_length) : 4096),
             flags)) > 0) {
    memcpy(response + total_length, buf, (size_t)bytes_received);
    total_length += bytes_received;
  }

  response[total_length] = '\0';
  RETURN_T_STRING(response, (int)total_length);
}

DECLARE_MODULE_METHOD(socket__setsockopt) {
  ENFORCE_ARG_COUNT(setsockopt, 4);
  ENFORCE_ARG_TYPE(setsockopt, 0, IS_NUMBER); // the socket id
  ENFORCE_ARG_TYPE(setsockopt, 1, IS_NUMBER); // the option id
  ENFORCE_ARG_TYPE(setsockopt, 2, IS_NUMBER); // the level

  int sock = AS_NUMBER(args[0]);
  int option = AS_NUMBER(args[1]);

  int level = (int)AS_NUMBER(args[2]);
  if (level == -1) {
    level = SOL_SOCKET;
  }

  z_value value = args[3];

  switch (option) {
    case SO_SNDTIMEO:
    case SO_RCVTIMEO: {
      ENFORCE_ARG_TYPE(setsockopt, 3, IS_NUMBER);

#ifdef _WIN32
      DWORD timeout = (DWORD)AS_NUMBER(value);
      RETURN_NUMBER(setsockopt(sock, level, option,
                               (const char *)&timeout, sizeof(timeout)));
#else
      int milliseconds = (int)AS_NUMBER(value);
      struct timeval tv = { (long)(milliseconds / 1000),
                            (int)((milliseconds % 1000) * 1000) };
      RETURN_NUMBER(setsockopt(sock, level, option, &tv, sizeof(tv)));
#endif
    }

#ifdef SO_LINGER
    case SO_LINGER: {
      ENFORCE_ARG_TYPE(setsockopt, 3, IS_NUMBER);
      int linger_secs = (int)AS_NUMBER(value);
      struct linger lg;
      lg.l_onoff  = (linger_secs > 0) ? 1 : 0;
      lg.l_linger = (linger_secs > 0) ? linger_secs : 0;
      RETURN_NUMBER(setsockopt(sock, level, SO_LINGER,
                               (const char *)&lg, sizeof(lg)));
    }
#endif

    default: {
      ENFORCE_ARG_TYPE(setsockopt, 3, IS_BOOL);
      int val = AS_BOOL(value) ? 1 : 0;
      RETURN_NUMBER(setsockopt(sock, level, option,
                               (const char *)&val, sizeof val));
    }
  }
}

DECLARE_MODULE_METHOD(socket__getsockopt) {
  ENFORCE_ARG_COUNT(getsockopt, 3);
  ENFORCE_ARG_TYPE(getsockopt, 0, IS_NUMBER); // the socket id
  ENFORCE_ARG_TYPE(getsockopt, 1, IS_NUMBER); // the option id
  ENFORCE_ARG_TYPE(getsockopt, 2, IS_NUMBER); // the level

  int sock = AS_NUMBER(args[0]);
  int option = AS_NUMBER(args[1]);

  int level = (int)AS_NUMBER(args[2]);
  if (level == -1) {
    level = SOL_SOCKET;
  }

  switch (option) {
    case SO_ERROR: {
      int so_error = 0;
      socklen_t len = (socklen_t)sizeof(so_error);
#ifndef _WIN32
      getsockopt(sock, level, SO_ERROR, &so_error, &len);
#else
      getsockopt(sock, level, SO_ERROR, (char *)&so_error, &len);
#endif
      if (so_error == 0) RETURN_NIL;
#ifdef _WIN32
      RETURN_STRING(strerror_socket(so_error));
#else
      RETURN_STRING(strerror(so_error));
#endif
    }

    case SO_SNDTIMEO:
    case SO_RCVTIMEO: {
#ifdef _WIN32
      DWORD timeout;
      int len = sizeof(timeout);
      if (getsockopt(sock, level, option, (char *)&timeout, &len) >= 0) {
        RETURN_NUMBER(timeout);
      }
#else
      struct timeval tv;
      socklen_t len = (socklen_t)sizeof(tv);
      getsockopt(sock, level, option, &tv, &len);
      if (len == sizeof(tv)) {
        RETURN_NUMBER((tv.tv_sec * 1000) + ((double)tv.tv_usec / 1000));
      }
#endif
      RETURN_NUMBER(-1);
    }

    default: {
      int so_result = 0;
      socklen_t len = (socklen_t)sizeof(so_result);
#ifndef _WIN32
      getsockopt(sock, level, option, &so_result, &len);
#else
      getsockopt(sock, level, option, (char *)&so_result, &len);
#endif
      if (len == sizeof(so_result)) {
        RETURN_NUMBER(so_result);
      }
      RETURN_NUMBER(-1);
    }
  }
}

DECLARE_MODULE_METHOD(socket__getsockinfo) {
  ENFORCE_ARG_COUNT(getsockinfo, 1);
  ENFORCE_ARG_TYPE(getsockinfo, 0, IS_NUMBER);

  int sock = AS_NUMBER(args[0]);

  struct sockaddr_storage ss;
  memset(&ss, 0, sizeof(ss));
  socklen_t ss_len = (socklen_t)sizeof(ss);

  z_obj_dict *dict = (z_obj_dict *)GC(new_dict(vm));

  if (getsockname(sock, (struct sockaddr *)&ss, &ss_len) < 0) {
    dict_add_entry(vm, dict, GC_L_STRING("address", 7), NIL_VAL);
    dict_add_entry(vm, dict, GC_L_STRING("ipv6",    4), NIL_VAL);
    dict_add_entry(vm, dict, GC_L_STRING("port",    4), NUMBER_VAL(-1));
    dict_add_entry(vm, dict, GC_L_STRING("family",  6), NUMBER_VAL(-1));
    RETURN_OBJ(dict);
  }

  char ip4[INET_ADDRSTRLEN]   = {0};
  char ip6[INET6_ADDRSTRLEN]  = {0};
  int  port   = -1;
  int  family = (int)ss.ss_family;

  if (ss.ss_family == AF_INET) {
    struct sockaddr_in *sa = (struct sockaddr_in *)&ss;
    inet_ntop(AF_INET, &sa->sin_addr, ip4, INET_ADDRSTRLEN);
    port = (int)ntohs(sa->sin_port);
  }
#ifdef AF_INET6
  else if (ss.ss_family == AF_INET6) {
    struct sockaddr_in6 *sa6 = (struct sockaddr_in6 *)&ss;
    inet_ntop(AF_INET6, &sa6->sin6_addr, ip6, INET6_ADDRSTRLEN);
    port = (int)ntohs(sa6->sin6_port);
  }
#endif

  /* Populate both address slots; whichever family is active will be non-empty */
  char *ip4_str = ALLOCATE(char, INET_ADDRSTRLEN);
  char *ip6_str = ALLOCATE(char, INET6_ADDRSTRLEN);
  memcpy(ip4_str, ip4, INET_ADDRSTRLEN);
  memcpy(ip6_str, ip6, INET6_ADDRSTRLEN);

  dict_add_entry(vm, dict, GC_L_STRING("address", 7), GC_TT_STRING(ip4_str));
  dict_add_entry(vm, dict, GC_L_STRING("ipv6",    4), GC_TT_STRING(ip6_str));
  dict_add_entry(vm, dict, GC_L_STRING("port",    4), NUMBER_VAL(port));
  dict_add_entry(vm, dict, GC_L_STRING("family",  6), NUMBER_VAL(family)); // [S6] host order, no ntohs
  RETURN_OBJ(dict);
}

DECLARE_MODULE_METHOD(socket__getaddrinfo) {
  ENFORCE_ARG_COUNT(getaddrinfo, 3);
  ENFORCE_ARG_TYPE(getaddrinfo, 0, IS_STRING);
  ENFORCE_ARG_TYPE(getaddrinfo, 2, IS_NUMBER);

  z_obj_string *addr = AS_STRING(args[0]);

  const char *type = "http";
  if (!IS_NIL(args[1])) {
    ENFORCE_ARG_TYPE(getaddrinfo, 1, IS_STRING);
    type = AS_C_STRING(args[1]);
  }
  int family = (int)AS_NUMBER(args[2]);

  struct addrinfo hints;
  memset(&hints, 0, sizeof(hints));
  hints.ai_socktype = SOCK_STREAM;
  hints.ai_family   = family;
  hints.ai_flags    = AI_CANONNAME; // populate ai_canonname

  struct addrinfo *res = NULL;
  if (getaddrinfo(addr->length > 0 ? addr->chars : NULL, type, &hints, &res) != 0
      || res == NULL) {
    RETURN_NIL;
  }

  struct addrinfo *rp;
  for (rp = res; rp != NULL; rp = rp->ai_next) {
    if (rp->ai_family != family) continue;

    z_obj_dict *dict = (z_obj_dict *)GC(new_dict(vm));

    /* [C3] Canonical name is now populated when available */
    if (rp->ai_canonname != NULL) {
      dict_add_entry(vm, dict, GC_L_STRING("canonical_name", 14),
                     GC_STRING(rp->ai_canonname));
    } else {
      dict_add_entry(vm, dict, GC_L_STRING("canonical_name", 14), NIL_VAL);
    }

    char *result = NULL;
    switch (family) {
      case AF_INET: {
        void *ptr = &((struct sockaddr_in *)rp->ai_addr)->sin_addr;
        result = ALLOCATE(char, INET_ADDRSTRLEN);
        inet_ntop(rp->ai_family, ptr, result, INET_ADDRSTRLEN);
        break;
      }
#ifdef AF_INET6
      case AF_INET6: {
        void *ptr = &((struct sockaddr_in6 *)rp->ai_addr)->sin6_addr;
        result = ALLOCATE(char, INET6_ADDRSTRLEN);
        inet_ntop(rp->ai_family, ptr, result, INET6_ADDRSTRLEN);
        break;
      }
#endif
      default: {
        result = ALLOCATE(char, 1);
        result[0] = '\0';
        break;
      }
    }

    dict_add_entry(vm, dict, GC_L_STRING("ip", 2), GC_TT_STRING(result));

    /* [S2b] Free the original head pointer, not the cursor */
    freeaddrinfo(res);
    RETURN_OBJ(dict);
  }

  /* No matching entry found */
  freeaddrinfo(res);
  RETURN_NIL;
}

DECLARE_MODULE_METHOD(socket__close) {
  ENFORCE_ARG_COUNT(close, 1);
  ENFORCE_ARG_TYPE(close, 0, IS_NUMBER);
  int sock = AS_NUMBER(args[0]);
  RETURN_NUMBER(closesocket(sock));
}

DECLARE_MODULE_METHOD(socket__shutdown) {
  ENFORCE_ARG_COUNT(shutdown, 2);
  ENFORCE_ARG_TYPE(shutdown, 0, IS_NUMBER); // socket id
  ENFORCE_ARG_TYPE(shutdown, 1, IS_NUMBER); // how  — [S8] was checking arg 0 twice
  RETURN_NUMBER(shutdown((int)AS_NUMBER(args[0]), (int)AS_NUMBER(args[1])));
}

DECLARE_MODULE_METHOD(socket__setblocking) {
  ENFORCE_ARG_COUNT(setblocking, 2);
  ENFORCE_ARG_TYPE(setblocking, 0, IS_NUMBER); // socket id
  ENFORCE_ARG_TYPE(setblocking, 1, IS_BOOL);   // blocking = true / non-blocking = false

  int sock    = (int)AS_NUMBER(args[0]);
  bool blocking = AS_BOOL(args[1]);

#ifndef _WIN32
  int flags = fcntl(sock, F_GETFL, 0);
  if (flags < 0) RETURN_NUMBER(-1);
  if (blocking) {
    flags &= ~O_NONBLOCK;
  } else {
    flags |= O_NONBLOCK;
  }
  RETURN_BOOL(fcntl(sock, F_SETFL, flags) != -1);
#else
  unsigned long mode = blocking ? 0UL : 1UL;
  RETURN_BOOL(ioctl(sock, FIONBIO, &mode) != -1);
#endif
}

DECLARE_MODULE_METHOD(socket__sendto) {
  ENFORCE_ARG_COUNT(sendto, 6);
  ENFORCE_ARG_TYPE(sendto, 0, IS_NUMBER); // socket id
  ENFORCE_ARG_TYPE(sendto, 2, IS_NUMBER); // flags
  ENFORCE_ARG_TYPE(sendto, 3, IS_STRING); // destination address
  ENFORCE_ARG_TYPE(sendto, 4, IS_NUMBER); // destination port
  ENFORCE_ARG_TYPE(sendto, 5, IS_NUMBER); // family

  int sock    = (int)AS_NUMBER(args[0]);
  z_value data = args[1];
  int flags   = (int)AS_NUMBER(args[2]);
  char *address = AS_C_STRING(args[3]);
  int port    = (int)AS_NUMBER(args[4]);
  int family  = (int)AS_NUMBER(args[5]);

  char *content = NULL;
  int length = 0;

  if (IS_STRING(data)) {
    content = AS_STRING(data)->chars;
    length  = AS_STRING(data)->length;
  } else if (IS_BYTES(data)) {
    content = (char *)AS_BYTES(data)->bytes.bytes;
    length  = AS_BYTES(data)->bytes.count;
  } else {
    z_obj_string *s = value_to_string(vm, data);
    content = s->chars;
    length  = s->length;
  }

#ifndef _WIN32
#ifdef MSG_NOSIGNAL
  flags |= MSG_NOSIGNAL;
#endif
#endif

  char port_str[16];
  snprintf(port_str, sizeof(port_str), "%d", port);

  struct addrinfo hints; memset(&hints, 0, sizeof(hints));
  hints.ai_family   = family;
  hints.ai_socktype = SOCK_DGRAM;

  struct addrinfo *res = NULL;
  if (getaddrinfo(address, port_str, &hints, &res) != 0 || res == NULL) {
    errno = EADDRNOTAVAIL;
    RETURN_NUMBER(-1);
  }

  int sent = (int)sendto(sock, content, (size_t)length, flags,
                         res->ai_addr, (socklen_t)res->ai_addrlen);
  freeaddrinfo(res);
  RETURN_NUMBER(sent);
}

DECLARE_MODULE_METHOD(socket__recvfrom) {
  ENFORCE_ARG_COUNT(recvfrom, 3);
  ENFORCE_ARG_TYPE(recvfrom, 0, IS_NUMBER); // socket id
  ENFORCE_ARG_TYPE(recvfrom, 1, IS_NUMBER); // max bytes to read
  ENFORCE_ARG_TYPE(recvfrom, 2, IS_NUMBER); // flags

  int sock   = (int)AS_NUMBER(args[0]);
  int length = (int)AS_NUMBER(args[1]);
  int flags  = (int)AS_NUMBER(args[2]);

  if (length <= 0) length = BIGSIZ;

  struct sockaddr_storage ss; memset(&ss, 0, sizeof(ss));
  socklen_t ss_len = (socklen_t)sizeof(ss);

  char *buf = ALLOCATE(char, length + 1);
  int received = (int)recvfrom(sock, buf, (size_t)length, flags,
                               (struct sockaddr *)&ss, &ss_len);
  if (received < 0) {
    FREE_ARRAY(char, buf, length + 1);
    RETURN_NUMBER(-1);
  }
  buf[received] = '\0';

  /* Resolve sender address */
  char sender_ip[INET6_ADDRSTRLEN] = {0};
  int  sender_port = 0;

  if (ss.ss_family == AF_INET) {
    struct sockaddr_in *sa = (struct sockaddr_in *)&ss;
    inet_ntop(AF_INET, &sa->sin_addr, sender_ip, INET_ADDRSTRLEN);
    sender_port = (int)ntohs(sa->sin_port);
  }
#ifdef AF_INET6
  else if (ss.ss_family == AF_INET6) {
    struct sockaddr_in6 *sa6 = (struct sockaddr_in6 *)&ss;
    inet_ntop(AF_INET6, &sa6->sin6_addr, sender_ip, INET6_ADDRSTRLEN);
    sender_port = (int)ntohs(sa6->sin6_port);
  }
#endif

  z_obj_list *result = (z_obj_list *)GC(new_list(vm));
  write_list(vm, result, GC_TT_STRING(buf));

  char *sender_str = ALLOCATE(char, INET6_ADDRSTRLEN);
  memcpy(sender_str, sender_ip, INET6_ADDRSTRLEN);
  write_list(vm, result, GC_TT_STRING(sender_str));
  write_list(vm, result, NUMBER_VAL(sender_port));

  RETURN_OBJ(result);
}

void __socket_module_unload(z_vm *vm) {
#ifdef _WIN32
  WSACleanup();
#endif
}

void __socket_module_preloader(z_vm *vm) {
#ifdef _WIN32
  WSADATA wsa_data;
  int i_result = WSAStartup(MAKEWORD(2, 2), &wsa_data);
  if (i_result != NO_ERROR) {
    errno = i_result;
    return;
  }

  if (LOBYTE(wsa_data.wVersion) != 2 || HIBYTE(wsa_data.wVersion) != 2) {
    WSACleanup();
    return;
  }
#else
# ifdef SIGPIPE
  signal(SIGPIPE, SIG_IGN);
# endif
#endif
}


/** START SOCKET CONSTANTS */

//  stream socket
z_value __socket_SOCK_STREAM(z_vm *vm) {
#ifdef SOCK_STREAM
  return NUMBER_VAL(SOCK_STREAM);
#else
  return NUMBER_VAL(-1);
#endif
}

//  datagram socket
z_value __socket_SOCK_DGRAM(z_vm *vm) {
#ifdef SOCK_DGRAM
  return NUMBER_VAL(SOCK_DGRAM);
#else
  return NUMBER_VAL(-1);
#endif
}

//  raw-protocol interface
z_value __socket_SOCK_RAW(z_vm *vm) {
#ifdef SOCK_RAW
  return NUMBER_VAL(SOCK_RAW);
#else
  return NUMBER_VAL(-1);
#endif
}

//  reliably-delivered message
z_value __socket_SOCK_RDM(z_vm *vm) {
#ifdef SOCK_RDM
  return NUMBER_VAL(SOCK_RDM);
#else
  return NUMBER_VAL(-1);
#endif
}

//  sequenced packet stream
z_value __socket_SOCK_SEQPACKET(z_vm *vm) {
#ifdef SOCK_SEQPACKET
  return NUMBER_VAL(SOCK_SEQPACKET);
#else
  return NUMBER_VAL(-1);
#endif
}


//  turn on debugging info recording
z_value __socket_SO_DEBUG(z_vm *vm) {
#ifdef SO_DEBUG
  return NUMBER_VAL(SO_DEBUG);
#else
  return NUMBER_VAL(-1);
#endif
}

//  socket has had listen()
z_value __socket_SO_ACCEPTCONN(z_vm *vm) {
#ifdef SO_ACCEPTCONN
  return NUMBER_VAL(SO_ACCEPTCONN);
#else
  return NUMBER_VAL(-1);
#endif
}

//  allow local address reuse
z_value __socket_SO_REUSEADDR(z_vm *vm) {
#ifdef SO_REUSEADDR
  return NUMBER_VAL(SO_REUSEADDR);
#else
  return NUMBER_VAL(-1);
#endif
}

//  keep connections alive
z_value __socket_SO_KEEPALIVE(z_vm *vm) {
#ifdef SO_KEEPALIVE
  return NUMBER_VAL(SO_KEEPALIVE);
#else
  return NUMBER_VAL(-1);
#endif
}

//  just use interface addresses
z_value __socket_SO_DONTROUTE(z_vm *vm) {
#ifdef SO_DONTROUTE
  return NUMBER_VAL(SO_DONTROUTE);
#else
  return NUMBER_VAL(-1);
#endif
}

//  permit sending of broadcast msgs
z_value __socket_SO_BROADCAST(z_vm *vm) {
#ifdef SO_BROADCAST
  return NUMBER_VAL(SO_BROADCAST);
#else
  return NUMBER_VAL(-1);
#endif
}

//  bypass hardware when possible
z_value __socket_SO_USELOOPBACK(z_vm *vm) {
#ifdef SO_USELOOPBACK
  return NUMBER_VAL(SO_USELOOPBACK);
#else
  return NUMBER_VAL(-1);
#endif
}

//  linger on close if data present (in ticks)
z_value __socket_SO_LINGER(z_vm *vm) {
#ifdef SO_LINGER
  return NUMBER_VAL(SO_LINGER);
#else
  return NUMBER_VAL(-1);
#endif
}

//  leave received OOB data in line
z_value __socket_SO_OOBINLINE(z_vm *vm) {
#ifdef SO_OOBINLINE
  return NUMBER_VAL(SO_OOBINLINE);
#else
  return NUMBER_VAL(-1);
#endif
}

//  allow local address & port reuse
z_value __socket_SO_REUSEPORT(z_vm *vm) {
#ifdef SO_REUSEPORT
  return NUMBER_VAL(SO_REUSEPORT);
#else
  return NUMBER_VAL(-1);
#endif
}

//  timestamp received dgram traffic
z_value __socket_SO_TIMESTAMP(z_vm *vm) {
#ifdef SO_TIMESTAMP
  return NUMBER_VAL(SO_TIMESTAMP);
#else
  return NUMBER_VAL(-1);
#endif
}


//  send buffer size
z_value __socket_SO_SNDBUF(z_vm *vm) {
#ifdef SO_SNDBUF
  return NUMBER_VAL(SO_SNDBUF);
#else
  return NUMBER_VAL(-1);
#endif
}

//  receive buffer size
z_value __socket_SO_RCVBUF(z_vm *vm) {
#ifdef SO_RCVBUF
  return NUMBER_VAL(SO_RCVBUF);
#else
  return NUMBER_VAL(-1);
#endif
}

//  send low-water mark
z_value __socket_SO_SNDLOWAT(z_vm *vm) {
#ifdef SO_SNDLOWAT
  return NUMBER_VAL(SO_SNDLOWAT);
#else
  return NUMBER_VAL(-1);
#endif
}

//  receive low-water mark
z_value __socket_SO_RCVLOWAT(z_vm *vm) {
#ifdef SO_RCVLOWAT
  return NUMBER_VAL(SO_RCVLOWAT);
#else
  return NUMBER_VAL(-1);
#endif
}

//  send timeout
z_value __socket_SO_SNDTIMEO(z_vm *vm) {
#ifdef SO_SNDTIMEO
  return NUMBER_VAL(SO_SNDTIMEO);
#else
  return NUMBER_VAL(-1);
#endif
}

//  receive timeout
z_value __socket_SO_RCVTIMEO(z_vm *vm) {
#ifdef SO_RCVTIMEO
  return NUMBER_VAL(SO_RCVTIMEO);
#else
  return NUMBER_VAL(-1);
#endif
}

//  get error status and clear
z_value __socket_SO_ERROR(z_vm *vm) {
#ifdef SO_ERROR
  return NUMBER_VAL(SO_ERROR);
#else
  return NUMBER_VAL(-1);
#endif
}

//  get socket type
z_value __socket_SO_TYPE(z_vm *vm) {
#ifdef SO_TYPE
  return NUMBER_VAL(SO_TYPE);
#else
  return NUMBER_VAL(-1);
#endif
}



//  options for socket level
z_value __socket_SOL_SOCKET(z_vm *vm) {
#ifdef SOL_SOCKET
  return NUMBER_VAL(SOL_SOCKET);
#else
  return NUMBER_VAL(-1);
#endif
}


//  unspecified
z_value __socket_AF_UNSPEC(z_vm *vm) {
#ifdef AF_UNSPEC
  return NUMBER_VAL(AF_UNSPEC);
#else
  return NUMBER_VAL(-1);
#endif
}

//  local to host (pipes)
z_value __socket_AF_UNIX(z_vm *vm) {
#ifdef AF_UNIX
  return NUMBER_VAL(AF_UNIX);
#else
  return NUMBER_VAL(-1);
#endif
}

//  same as AF_UNIX
z_value __socket_AF_LOCAL(z_vm *vm) {
#ifdef AF_LOCAL
  return NUMBER_VAL(AF_LOCAL);
#else
  return NUMBER_VAL(AF_UNIX);
#endif
}

//  internetwork: UDP, TCP, etc.
z_value __socket_AF_INET(z_vm *vm) {
#ifdef AF_INET
  return NUMBER_VAL(AF_INET);
#else
  return NUMBER_VAL(-1);
#endif
}

//  arpanet imp addresses
z_value __socket_AF_IMPLINK(z_vm *vm) {
#ifdef AF_IMPLINK
  return NUMBER_VAL(AF_IMPLINK);
#else
  return NUMBER_VAL(-1);
#endif
}

//  pup protocols: e.g. BSP
z_value __socket_AF_PUP(z_vm *vm) {
#ifdef AF_PUP
  return NUMBER_VAL(AF_PUP);
#else
  return NUMBER_VAL(-1);
#endif
}

//  mit CHAOS protocols
z_value __socket_AF_CHAOS(z_vm *vm) {
#ifdef AF_CHAOS
  return NUMBER_VAL(AF_CHAOS);
#else
  return NUMBER_VAL(-1);
#endif
}

//  XEROX NS protocols
z_value __socket_AF_NS(z_vm *vm) {
#ifdef AF_NS
  return NUMBER_VAL(AF_NS);
#else
  return NUMBER_VAL(-1);
#endif
}

//  ISO protocols
z_value __socket_AF_ISO(z_vm *vm) {
#ifdef AF_ISO
  return NUMBER_VAL(AF_ISO);
#else
  return NUMBER_VAL(-1);
#endif
}

//  OSI protocols (same as ISO)
z_value __socket_AF_OSI(z_vm *vm) {
#ifdef AF_OSI
  return NUMBER_VAL(AF_OSI);
#else
  return NUMBER_VAL(-1);
#endif
}

//  European computer manufacturers
z_value __socket_AF_ECMA(z_vm *vm) {
#ifdef AF_ECMA
  return NUMBER_VAL(AF_ECMA);
#else
  return NUMBER_VAL(-1);
#endif
}

//  datakit protocols
z_value __socket_AF_DATAKIT(z_vm *vm) {
#ifdef AF_DATAKIT
  return NUMBER_VAL(AF_DATAKIT);
#else
  return NUMBER_VAL(-1);
#endif
}

//  CCITT protocols, X.25 etc
z_value __socket_AF_CCITT(z_vm *vm) {
#ifdef AF_CCITT
  return NUMBER_VAL(AF_CCITT);
#else
  return NUMBER_VAL(-1);
#endif
}

//  IBM SNA
z_value __socket_AF_SNA(z_vm *vm) {
#ifdef AF_SNA
  return NUMBER_VAL(AF_SNA);
#else
  return NUMBER_VAL(-1);
#endif
}

//  DECnet
z_value __socket_AF_DECnet(z_vm *vm) {
#ifdef AF_DECnet
  return NUMBER_VAL(AF_DECnet);
#else
  return NUMBER_VAL(-1);
#endif
}

//  DEC Direct data link interface
z_value __socket_AF_DLI(z_vm *vm) {
#ifdef AF_DLI
  return NUMBER_VAL(AF_DLI);
#else
  return NUMBER_VAL(-1);
#endif
}

//  LAT
z_value __socket_AF_LAT(z_vm *vm) {
#ifdef AF_LAT
  return NUMBER_VAL(AF_LAT);
#else
  return NUMBER_VAL(-1);
#endif
}

//  NSC Hyperchannel
z_value __socket_AF_HYLINK(z_vm *vm) {
#ifdef AF_HYLINK
  return NUMBER_VAL(AF_HYLINK);
#else
  return NUMBER_VAL(-1);
#endif
}

//  AppleTalk
z_value __socket_AF_APPLETALK(z_vm *vm) {
#ifdef AF_APPLETALK
  return NUMBER_VAL(AF_APPLETALK);
#else
  return NUMBER_VAL(-1);
#endif
}

//  ipv6
z_value __socket_AF_INET6(z_vm *vm) {
#ifdef AF_INET6
  return NUMBER_VAL(AF_INET6);
#else
  return NUMBER_VAL(-1);
#endif
}


//   Dummy protocol for TCP.
z_value __socket_IPPROTO_IP(z_vm *vm) {
#ifdef IPPROTO_IP
  return NUMBER_VAL(IPPROTO_IP);
#else
  return NUMBER_VAL(-1);
#endif
}

//   Internet Control Message Protocol.
z_value __socket_IPPROTO_ICMP(z_vm *vm) {
  return NUMBER_VAL(IPPROTO_ICMP);
}

//   Internet Group Management Protocol.
z_value __socket_IPPROTO_IGMP(z_vm *vm) {
  return NUMBER_VAL(IPPROTO_IGMP);
}

//   IPIP tunnels (older KA9Q tunnels use 94).
z_value __socket_IPPROTO_IPIP(z_vm *vm) {
#ifdef IPPROTO_IPIP
  return NUMBER_VAL(IPPROTO_IPIP);
#else
  return NUMBER_VAL(IPPROTO_IPV4);
#endif
}

//   Transmission Control Protocol.
z_value __socket_IPPROTO_TCP(z_vm *vm) {
  return NUMBER_VAL(IPPROTO_TCP);
}

//   Exterior Gateway Protocol.
z_value __socket_IPPROTO_EGP(z_vm *vm) {
#ifdef IPPROTO_EGP
  return NUMBER_VAL(IPPROTO_EGP);
#else
  return NUMBER_VAL(-1);
#endif
}

//   PUP protocol.
z_value __socket_IPPROTO_PUP(z_vm *vm) {
  return NUMBER_VAL(IPPROTO_PUP);
}

//   User Datagram Protocol.
z_value __socket_IPPROTO_UDP(z_vm *vm) {
  return NUMBER_VAL(IPPROTO_UDP);
}

//   XNS IDP protocol.
z_value __socket_IPPROTO_IDP(z_vm *vm) {
  return NUMBER_VAL(IPPROTO_IDP);
}

//   SO Transport Protocol Class 4.
z_value __socket_IPPROTO_TP(z_vm *vm) {
#ifdef IPPROTO_TP
  return NUMBER_VAL(IPPROTO_TP);
#else
  return NUMBER_VAL(-1);
#endif
}

//   Datagram Congestion Control Protocol.
z_value __socket_IPPROTO_DCCP(z_vm *vm) {
#ifdef IPPROTO_DCCP
  return NUMBER_VAL(IPPROTO_DCCP);
#else
  return NUMBER_VAL(-1);
#endif
}

//   IPv6 header.
z_value __socket_IPPROTO_IPV6(z_vm *vm) {
  return NUMBER_VAL(IPPROTO_IPV6);
}

//   Reservation Protocol.
z_value __socket_IPPROTO_RSVP(z_vm *vm) {
#ifdef IPPROTO_RSVP
  return NUMBER_VAL(IPPROTO_RSVP);
#else
  return NUMBER_VAL(-1);
#endif
}

//   General Routing Encapsulation.
z_value __socket_IPPROTO_GRE(z_vm *vm) {
#ifdef IPPROTO_GRE
  return NUMBER_VAL(IPPROTO_GRE);
#else
  return NUMBER_VAL(-1);
#endif
}

//   encapsulating security payload.
z_value __socket_IPPROTO_ESP(z_vm *vm) {
  return NUMBER_VAL(IPPROTO_ESP);
}

//   authentication header.
z_value __socket_IPPROTO_AH(z_vm *vm) {
  return NUMBER_VAL(IPPROTO_AH);
}

//   Multicast Transport Protocol.
z_value __socket_IPPROTO_MTP(z_vm *vm) {
#ifdef IPPROTO_MTP
  return NUMBER_VAL(IPPROTO_MTP);
#else
  return NUMBER_VAL(-1);
#endif
}

//   IP option pseudo header for BEET.
z_value __socket_IPPROTO_BEETPH(z_vm *vm) {
#ifdef IPPROTO_BEETPH
  return NUMBER_VAL(IPPROTO_BEETPH);
#else
  return NUMBER_VAL(-1);
#endif
}

//   Encapsulation Header.
z_value __socket_IPPROTO_ENCAP(z_vm *vm) {
#ifdef IPPROTO_ENCAP
  return NUMBER_VAL(IPPROTO_ENCAP);
#else
  return NUMBER_VAL(-1);
#endif
}

//   Protocol Independent Multicast.
z_value __socket_IPPROTO_PIM(z_vm *vm) {
  return NUMBER_VAL(IPPROTO_PIM);
}

//   Compression Header Protocol.
z_value __socket_IPPROTO_COMP(z_vm *vm) {
#ifdef IPPROTO_COMP
  return NUMBER_VAL(IPPROTO_COMP);
#else
  return NUMBER_VAL(-1);
#endif
}

//   Stream Control Transmission Protocol.
z_value __socket_IPPROTO_SCTP(z_vm *vm) {
  return NUMBER_VAL(IPPROTO_SCTP);
}

//   UDP-Lite protocol.
z_value __socket_IPPROTO_UDPLITE(z_vm *vm) {
#ifdef IPPROTO_UDPLITE
  return NUMBER_VAL(IPPROTO_UDPLITE);
#else
  return NUMBER_VAL(-1);
#endif
}

//   MPLS in IP.
z_value __socket_IPPROTO_MPLS(z_vm *vm) {
#ifdef IPPROTO_MPLS
  return NUMBER_VAL(IPPROTO_MPLS);
#else
  return NUMBER_VAL(-1);
#endif
}

//   Raw IP packets.
z_value __socket_IPPROTO_RAW(z_vm *vm) {
  return NUMBER_VAL(IPPROTO_RAW);
}

//  max IP proto
z_value __socket_IPPROTO_MAX(z_vm *vm) {
  return NUMBER_VAL(IPPROTO_MAX);
}


//  shut down the reading side
z_value __socket_SHUT_RD(z_vm *vm) {
#ifdef SHUT_RD
  return NUMBER_VAL(SHUT_RD);
#else
  return NUMBER_VAL(-1);
#endif
}

//  shut down the writing side
z_value __socket_SHUT_WR(z_vm *vm) {
#ifdef SHUT_WR
  return NUMBER_VAL(SHUT_WR);
#else
  return NUMBER_VAL(-1);
#endif
}

//  shut down both sides
z_value __socket_SHUT_RDWR(z_vm *vm) {
#ifdef SHUT_RDWR
  return NUMBER_VAL(SHUT_RDWR);
#else
  return NUMBER_VAL(-1);
#endif
}


//  Maximum queue length specifiable by listen.
z_value __socket_SOMAXCONN(z_vm *vm) {
#ifdef SOMAXCONN
  return NUMBER_VAL(SOMAXCONN);
#else
  return NUMBER_VAL(-1);
#endif
}

z_value __socket_TCP_NODELAY(z_vm *vm) {
#ifdef TCP_NODELAY
  return NUMBER_VAL(TCP_NODELAY);
#else
  return NUMBER_VAL(-1);
#endif
}

z_value __socket_TCP_FASTOPEN(z_vm *vm) {
#ifdef TCP_FASTOPEN
  return NUMBER_VAL(TCP_FASTOPEN);
#else
  return NUMBER_VAL(-1);
#endif
}

/** END SOCKET CONSTANTS */



CREATE_MODULE_LOADER(socket) {
  static z_func_reg module_functions[] = {
      {"create",      false, GET_MODULE_METHOD(socket__create)},
      {"connect",     false, GET_MODULE_METHOD(socket__connect)},
      {"send",        false, GET_MODULE_METHOD(socket__send)},
      {"recv",        false, GET_MODULE_METHOD(socket__recv)},
      {"read",        false, GET_MODULE_METHOD(socket__read)},
      {"setsockopt",  false, GET_MODULE_METHOD(socket__setsockopt)},
      {"getsockopt",  false, GET_MODULE_METHOD(socket__getsockopt)},
      {"bind",        false, GET_MODULE_METHOD(socket__bind)},
      {"listen",      false, GET_MODULE_METHOD(socket__listen)},
      {"accept",      false, GET_MODULE_METHOD(socket__accept)},
      {"error",       false, GET_MODULE_METHOD(socket__error)},
      {"close",       false, GET_MODULE_METHOD(socket__close)},
      {"shutdown",    false, GET_MODULE_METHOD(socket__shutdown)},
      {"getsockinfo", false, GET_MODULE_METHOD(socket__getsockinfo)},
      {"getaddrinfo", false, GET_MODULE_METHOD(socket__getaddrinfo)},
      {"setblocking", false, GET_MODULE_METHOD(socket__setblocking)},
      {"sendto",      false, GET_MODULE_METHOD(socket__sendto)},
      {"recvfrom",    false, GET_MODULE_METHOD(socket__recvfrom)},
      {NULL,          false, NULL},
  };

  static z_field_reg socket_module_fields[] = {
      /**
       * Types
       */
      {"SOCK_STREAM",    true, __socket_SOCK_STREAM},
      {"SOCK_DGRAM",     true, __socket_SOCK_DGRAM},
      {"SOCK_RAW",       true, __socket_SOCK_RAW},
      {"SOCK_RDM",       true, __socket_SOCK_RDM},
      {"SOCK_SEQPACKET", true, __socket_SOCK_SEQPACKET},

      /**
       * Option flags per-socket.
       */
      {"SO_DEBUG",       true, __socket_SO_DEBUG},
      {"SO_ACCEPTCONN",  true, __socket_SO_ACCEPTCONN},
      {"SO_REUSEADDR",   true, __socket_SO_REUSEADDR},
      {"SO_KEEPALIVE",   true, __socket_SO_KEEPALIVE},
      {"SO_DONTROUTE",   true, __socket_SO_DONTROUTE},
      {"SO_BROADCAST",   true, __socket_SO_BROADCAST},
      {"SO_USELOOPBACK", true, __socket_SO_USELOOPBACK},
      {"SO_LINGER",      true, __socket_SO_LINGER},
      {"SO_OOBINLINE",   true, __socket_SO_OOBINLINE},
      {"SO_REUSEPORT",   true, __socket_SO_REUSEPORT},
      {"SO_TIMESTAMP",   true, __socket_SO_TIMESTAMP},

      /**
       * Additional options, not kept in so_options.
       */
      {"SO_SNDBUF",      true, __socket_SO_SNDBUF},
      {"SO_RCVBUF",      true, __socket_SO_RCVBUF},
      {"SO_SNDLOWAT",    true, __socket_SO_SNDLOWAT},
      {"SO_RCVLOWAT",    true, __socket_SO_RCVLOWAT},
      {"SO_SNDTIMEO",    true, __socket_SO_SNDTIMEO},
      {"SO_RCVTIMEO",    true, __socket_SO_RCVTIMEO},
      {"SO_ERROR",       true, __socket_SO_ERROR},
      {"SO_TYPE",        true, __socket_SO_TYPE},

      {"SOL_SOCKET",     true, __socket_SOL_SOCKET},

      /**
       * Address families.
       */
      {"AF_UNSPEC",      true, __socket_AF_UNSPEC},
      {"AF_UNIX",        true, __socket_AF_UNIX},
      {"AF_LOCAL",       true, __socket_AF_LOCAL},
      {"AF_INET",        true, __socket_AF_INET},
      {"AF_IMPLINK",     true, __socket_AF_IMPLINK},
      {"AF_PUP",         true, __socket_AF_PUP},
      {"AF_CHAOS",       true, __socket_AF_CHAOS},
      {"AF_NS",          true, __socket_AF_NS},
      {"AF_ISO",         true, __socket_AF_ISO},
      {"AF_OSI",         true, __socket_AF_OSI},
      {"AF_ECMA",        true, __socket_AF_ECMA},
      {"AF_DATAKIT",     true, __socket_AF_DATAKIT},
      {"AF_CCITT",       true, __socket_AF_CCITT},
      {"AF_SNA",         true, __socket_AF_SNA},
      {"AF_DECnet",      true, __socket_AF_DECnet},
      {"AF_DLI",         true, __socket_AF_DLI},
      {"AF_LAT",         true, __socket_AF_LAT},
      {"AF_HYLINK",      true, __socket_AF_HYLINK},
      {"AF_APPLETALK",   true, __socket_AF_APPLETALK},
      {"AF_INET6",       true, __socket_AF_INET6},

      /**
       * Standard well-defined IP protocols.
       */
      {"IPPROTO_IP",     true, __socket_IPPROTO_IP},
      {"IPPROTO_ICMP",   true, __socket_IPPROTO_ICMP},
      {"IPPROTO_IGMP",   true, __socket_IPPROTO_IGMP},
      {"IPPROTO_IPIP",   true, __socket_IPPROTO_IPIP},
      {"IPPROTO_TCP",    true, __socket_IPPROTO_TCP},
      {"IPPROTO_EGP",    true, __socket_IPPROTO_EGP},
      {"IPPROTO_PUP",    true, __socket_IPPROTO_PUP},
      {"IPPROTO_UDP",    true, __socket_IPPROTO_UDP},
      {"IPPROTO_IDP",    true, __socket_IPPROTO_IDP},
      {"IPPROTO_TP",     true, __socket_IPPROTO_TP},
      {"IPPROTO_DCCP",   true, __socket_IPPROTO_DCCP},
      {"IPPROTO_IPV6",   true, __socket_IPPROTO_IPV6},
      {"IPPROTO_RSVP",   true, __socket_IPPROTO_RSVP},
      {"IPPROTO_GRE",    true, __socket_IPPROTO_GRE},
      {"IPPROTO_ESP",    true, __socket_IPPROTO_ESP},
      {"IPPROTO_AH",     true, __socket_IPPROTO_AH},
      {"IPPROTO_MTP",    true, __socket_IPPROTO_MTP},
      {"IPPROTO_BEETPH", true, __socket_IPPROTO_BEETPH},
      {"IPPROTO_ENCAP",  true, __socket_IPPROTO_ENCAP},
      {"IPPROTO_PIM",    true, __socket_IPPROTO_PIM},
      {"IPPROTO_COMP",   true, __socket_IPPROTO_COMP},
      {"IPPROTO_SCTP",   true, __socket_IPPROTO_SCTP},
      {"IPPROTO_UDPLITE",true, __socket_IPPROTO_UDPLITE},
      {"IPPROTO_MPLS",   true, __socket_IPPROTO_MPLS},
      {"IPPROTO_RAW",    true, __socket_IPPROTO_RAW},
      {"IPPROTO_MAX",    true, __socket_IPPROTO_MAX},

      /**
       * TCP Options
       */
      {"TCP_NODELAY",    true, __socket_TCP_NODELAY},
      {"TCP_FASTOPEN",    true, __socket_TCP_FASTOPEN},

      /**
       * howto arguments for shutdown(2), specified by Posix.1g.
       */
      {"SHUT_RD",        true, __socket_SHUT_RD},
      {"SHUT_WR",        true, __socket_SHUT_WR},
      {"SHUT_RDWR",      true, __socket_SHUT_RDWR},

      /**
       * Maximum queue length specifiable by listen.
       */
      {"SOMAXCONN",      true, __socket_SOMAXCONN},

      {NULL,             false, NULL},
  };

  static z_module_reg module = {
      .name      = "_socket",
      .fields    = socket_module_fields,
      .functions = module_functions,
      .classes   = NULL,
      .preloader = &__socket_module_preloader,
      .unloader  = &__socket_module_unload
  };
  return &module;
}

#undef BIGSIZ
#undef SMALLSIZ