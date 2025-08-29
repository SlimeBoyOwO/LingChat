/**
 * 清空用户数据工具
 * 此脚本用于清除所有存储在浏览器中的用户登录信息
 */

// 清空所有用户登录数据
function clearAllUserData() {
  // 清除用户信息
  localStorage.removeItem("user");
  
  // 清除记住登录的相关信息
  localStorage.removeItem("remembered_email");
  localStorage.removeItem("remembered_password");
  localStorage.removeItem("remember_me");
  
  // 清除其他可能与用户相关的数据
  localStorage.removeItem("chat_history");
  
  console.log("已清空所有用户登录信息");
  return "已成功清空所有用户登录信息";
}

// 立即执行清除操作
const result = clearAllUserData();

// 创建结果显示元素
document.addEventListener("DOMContentLoaded", () => {
  const body = document.body;
  const resultDiv = document.createElement("div");
  resultDiv.style.position = "fixed";
  resultDiv.style.top = "50%";
  resultDiv.style.left = "50%";
  resultDiv.style.transform = "translate(-50%, -50%)";
  resultDiv.style.padding = "20px";
  resultDiv.style.backgroundColor = "rgba(255, 255, 255, 0.9)";
  resultDiv.style.backdropFilter = "blur(10px)";
  resultDiv.style.borderRadius = "10px";
  resultDiv.style.boxShadow = "0 4px 20px rgba(0, 0, 0, 0.2)";
  resultDiv.style.color = "#333";
  resultDiv.style.fontFamily = "Arial, sans-serif";
  resultDiv.style.zIndex = "9999";
  
  resultDiv.innerHTML = `
    <h2 style="margin-top:0;color:#4a86cf;">用户数据清除结果</h2>
    <p>${result}</p>
    <p>您现在可以关闭此页面，然后重新登录。</p>
    <div style="text-align:center;margin-top:20px;">
      <button id="goToLogin" style="padding:10px 20px;background:#4a86cf;color:white;border:none;border-radius:5px;cursor:pointer;">
        前往登录页面
      </button>
    </div>
  `;
  
  body.appendChild(resultDiv);
  
  // 添加按钮事件
  document.getElementById("goToLogin").addEventListener("click", () => {
    window.location.href = "/login";
  });
}); 