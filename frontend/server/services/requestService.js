function handlePythonMessage(message, pendingRequests) {
  try {
    const data = JSON.parse(message);

    if (data.type === "auth_response" && pendingRequests[data.requestId]) {
      const { resolve, timeout } = pendingRequests[data.requestId];
      clearTimeout(timeout);
      resolve(data);
      delete pendingRequests[data.requestId];
      return data;
    }

    if (data.audioFile) {
      data.audioUrl = `/audio/${data.audioFile}`;
    }

    return data;
  } catch (e) {
    console.error("Error processing Python response:", e);
    return null;
  }
}

module.exports = {
  handlePythonMessage,
};
