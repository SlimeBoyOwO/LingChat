// auth-check.js - Script to check authentication status before allowing access to pages

document.addEventListener("DOMContentLoaded", () => {
  // Skip login check for the login page
  if (window.location.pathname.includes("login")) {
    return;
  }

  // Check if user is logged in
  const userData = localStorage.getItem("user");
  let isLoggedIn = false;

  if (userData) {
    try {
      const user = JSON.parse(userData);
      isLoggedIn = user && user.isLoggedIn;
    } catch (error) {
      console.error("Error parsing user data:", error);
      localStorage.removeItem("user");
    }
  }

  // If not logged in, redirect to login page
  if (!isLoggedIn) {
    window.location.href = "/login";
  }
});

// Export a function to check login status
export function checkLoginStatus() {
  const userData = localStorage.getItem("user");
  
  if (userData) {
    try {
      const user = JSON.parse(userData);
      return user && user.isLoggedIn;
    } catch (error) {
      console.error("Error parsing user data:", error);
      localStorage.removeItem("user");
    }
  }
  
  return false;
}

// Function to check if user is admin
export function isAdminUser() {
  const userData = localStorage.getItem("user");
  console.log("isAdminUser 被调用");
  
  if (userData) {
    try {
      const user = JSON.parse(userData);
      console.log("isAdminUser 解析用户数据:", user);
      const isAdmin = user && user.isLoggedIn && user.role === "admin";
      console.log("isAdminUser 判断结果:", isAdmin);
      return isAdmin;
    } catch (error) {
      console.error("isAdminUser 错误:", error);
    }
  } else {
    console.log("isAdminUser 未找到用户数据");
  }
  
  return false;
}

// Function to get the current user
export function getCurrentUser() {
  const userData = localStorage.getItem("user");
  
  if (userData) {
    try {
      return JSON.parse(userData);
    } catch (error) {
      console.error("Error parsing user data:", error);
      localStorage.removeItem("user");
    }
  }
  
  return null;
}

// Function to logout
export function logout() {
  localStorage.removeItem("user");
  window.location.href = "/login";
} 