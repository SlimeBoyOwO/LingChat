function switchLocation(isWs = false) {
  const protocol = window.location.protocol
  const host = window.location.host || 'localhost';
  if (isWs) {
    switch (protocol) {
      case "http:":
        return `ws://${host}`
      case "https:":
        return `wss://${host}`

      default:
        return `ws://${host}`
    }
  } else {
    return `${protocol}://${host}`
  }
}
