<template>
  <MenuPage>
    <!-- 连接状态指示器 -->
    <MenuItem title="🔗 更新服务连接状态" size="small">
      <div v-if="!backendConnected" class="connection-status error">
        <p>⚠️ 未连接到更新服务</p>
        <p>请确保 update_api.py 正在运行在端口 5001</p>
        <Button type="big" @click="checkBackendConnection">重试连接</Button>
      </div>
      <div v-else class="connection-status success">
        <p>✅ 已连接到更新服务</p>
      </div>
    </MenuItem>

    <!-- 当前版本信息 -->
    <MenuItem title="📋 当前版本信息" size="small">
      <div class="current-version">
        <p><strong>当前版本:</strong> {{ currentVersion }}</p>
        <p><strong>更新状态:</strong> {{ updateStatus }}</p>
        <p v-if="errorMessage" class="error-message">{{ errorMessage }}</p>
      </div>
    </MenuItem>

    <!-- 操作按钮 -->
    <MenuItem title="🔄 更新操作" size="small">
      <div class="action-buttons">
        <Button 
          type="big"
          @click="checkForUpdates" 
          :disabled="!backendConnected || isChecking || isDownloading || isRollingBack"
          class="left-button"
        >
          {{ isChecking ? '⏳ 检查中...' : '🔍 检查更新' }}
        </Button>
        
        <Button 
          type="big"
          @click="downloadAndApplyUpdate" 
          :disabled="!backendConnected || !updateAvailable || isChecking || isDownloading || isRollingBack"
          class="left-button"
        >
          {{ isDownloading ? '⏳ 下载中...' : '📥 下载并应用更新' }}
        </Button>
        
        <Button 
          type="big"
          @click="rollbackUpdate" 
          :disabled="!backendConnected || isChecking || isDownloading || isRollingBack"
          class="left-button danger"
        >
          {{ isRollingBack ? '⏳ 回滚中...' : '↩️ 回滚到上次备份' }}
        </Button>
      </div>
    </MenuItem>

    <!-- 更新信息 -->
    <MenuItem v-if="updateInfo" title="🆕 发现新版本" size="small">
      <div class="update-info">
        <p><strong>版本:</strong> {{ updateInfo.version }}</p>
        <p><strong>更新内容:</strong> {{ updateInfo.changelog || '无' }}</p>
      </div>
    </MenuItem>

    <!-- 进度条 -->
    <MenuItem v-if="showProgress" title="📊 更新进度" size="small">
      <div class="progress-container">
        <div class="progress-bar">
          <div 
            class="progress-fill" 
            :style="{ width: progress + '%' }"
          ></div>
        </div>
        <p class="progress-text">{{ progressMessage }}</p>
      </div>
    </MenuItem>

    <!-- 配置设置 -->
    <MenuItem title="⚙ 更新配置">
      <div class="config-item">
        <label>
          <input type="checkbox" v-model="config.auto_backup" @change="updateConfig">
          自动创建备份
        </label>
        <span class="config-help">应用更新前自动创建完整备份</span>
      </div>
    </MenuItem>

    <!-- 确认对话框 -->
    <div v-if="showRollbackDialog" class="dialog-overlay">
      <div class="dialog">
        <h3>确认回滚</h3>
        <p>确认回滚到上次备份吗？</p>
        <div class="dialog-actions">
          <Button type="big" @click="confirmRollback" class="danger">确认回滚</Button>
          <Button type="big" @click="cancelRollback" class="left-button">取消</Button>
        </div>
      </div>
    </div>
  </MenuPage>
</template>

<script>
import axios from 'axios';
import { MenuPage, MenuItem } from "../../ui";
import { Button } from "../../base";

export default {
  name: 'SettingsUpdate',
  components: {
    MenuPage,
    MenuItem,
    Button
  },
  data() {
    return {
      // API基础URL - 根据您的实际部署调整
      apiBaseUrl: 'http://localhost:5001/api/update',
      
      // 应用信息
      currentVersion: '未知',
      updateAvailable: false,
      
      // 更新状态
      updateStatus: 'idle',
      updateInfo: null,
      errorMessage: '',
      
      // 操作状态
      isChecking: false,
      isDownloading: false,
      isRollingBack: false,
      
      // 进度信息
      progress: 0,
      progressMessage: '',
      showProgress: false,
      
      // 配置
      config: {
        auto_backup: true
      },
      
      // 连接状态
      backendConnected: false,
      connectionRetries: 0,
      maxRetries: 5,
      
      // 对话框状态
      showRollbackDialog: false,
      
      // 轮询状态更新的定时器
      statusPolling: null
    };
  },
  
  mounted() {
    this.checkBackendConnection();
  },
  
  beforeUnmount() {
    this.stopStatusPolling();
  },
  
  computed: {
    isCheckingUpdates() {
      return this.updateStatus === 'checking';
    },
    
    isDownloadingUpdates() {
      return this.updateStatus === 'downloading';
    },
    
    isApplyingUpdates() {
      return this.updateStatus === 'applying';
    },
    
    isRollingBackUpdates() {
      return this.updateStatus === 'rolling_back';
    }
  },
  
  methods: {
    // 检查后端连接
    async checkBackendConnection() {
      try {
        const response = await axios.get(`${this.apiBaseUrl}/health`, { timeout: 5000 });
        if (response.data && response.data.status === 'ok') {
          this.backendConnected = true;
          this.connectionRetries = 0;
          this.errorMessage = '';
          this.loadAppInfo();
          this.loadConfig();
          this.startStatusPolling();
          console.log('成功连接到更新服务');
        }
      } catch (error) {
        console.error('无法连接到后端服务:', error);
        this.backendConnected = false;
        
        if (this.connectionRetries < this.maxRetries) {
          this.connectionRetries++;
          console.log(`重试连接 (${this.connectionRetries}/${this.maxRetries})...`);
          setTimeout(() => this.checkBackendConnection(), 2000);
        } else {
          this.errorMessage = `无法连接到更新服务。请确保 update_api.py 正在运行在端口 5001。错误: ${error.message}`;
          this.stopStatusPolling();
        }
      }
    },
    
    // 加载应用信息
    async loadAppInfo() {
      if (!this.backendConnected) return;
      
      try {
        const response = await axios.get(`${this.apiBaseUrl}/info`, { timeout: 5000 });
        if (response.data) {
          this.currentVersion = response.data.current_version || '未知';
          this.updateAvailable = response.data.update_available || false;
        }
      } catch (error) {
        console.error('获取应用信息失败:', error);
        this.handleApiError(error, '获取应用信息');
      }
    },
    
    // 加载配置
    async loadConfig() {
      if (!this.backendConnected) return;
      
      try {
        const response = await axios.get(`${this.apiBaseUrl}/config`, { timeout: 5000 });
        if (response.data) {
          this.config.auto_backup = response.data.auto_backup || true;
        }
      } catch (error) {
        console.error('获取配置失败:', error);
        this.handleApiError(error, '获取配置');
      }
    },
    
    // 更新配置
    async updateConfig() {
      if (!this.backendConnected) return;
      
      try {
        await axios.post(`${this.apiBaseUrl}/config`, this.config, { timeout: 5000 });
      } catch (error) {
        console.error('更新配置失败:', error);
        this.handleApiError(error, '更新配置');
      }
    },
    
    // 开始轮询状态
    startStatusPolling() {
      this.stopStatusPolling(); // 先停止现有的轮询
      this.statusPolling = setInterval(async () => {
        await this.getUpdateStatus();
      }, 1000); // 每秒更新一次状态
    },
    
    // 停止轮询状态
    stopStatusPolling() {
      if (this.statusPolling) {
        clearInterval(this.statusPolling);
        this.statusPolling = null;
      }
    },
    
    // 获取更新状态
    async getUpdateStatus() {
      if (!this.backendConnected) return;
      
      try {
        const response = await axios.get(`${this.apiBaseUrl}/status`, { timeout: 5000 });
        if (response.data) {
          const status = response.data;
          this.updateStatus = status.status || 'idle';
          this.progress = status.progress || 0;
          this.progressMessage = status.message || '';
          
          // 更新操作状态
          this.isChecking = this.updateStatus === 'checking';
          this.isDownloading = ['downloading', 'applying'].includes(this.updateStatus);
          this.isRollingBack = this.updateStatus === 'rolling_back';
          
          // 如果有错误信息
          if (status.error) {
            this.errorMessage = status.error;
          } else if (this.updateStatus !== 'error') {
            // 如果不是错误状态，清除错误消息
            this.errorMessage = '';
          }
          
          // 如果有更新信息
          if (status.update_info) {
            this.updateInfo = status.update_info;
            this.updateAvailable = true;
          }
          
          // 根据状态显示进度条
          this.showProgress = ['checking', 'downloading', 'applying', 'rolling_back'].includes(this.updateStatus);
          
          // 如果操作完成，重置状态并重新加载应用信息
          if (this.updateStatus === 'completed') {
            setTimeout(() => {
              this.loadAppInfo();
            }, 1000);
          }
        }
      } catch (error) {
        console.error('获取更新状态失败:', error);
        this.handleApiError(error, '获取更新状态');
      }
    },
    
    // 统一的API错误处理
    handleApiError(error, operation) {
      if (error.code === 'NETWORK_ERROR' || error.message.includes('Network Error')) {
        this.backendConnected = false;
        this.errorMessage = `网络错误: 无法连接到更新服务。请确保 update_api.py 正在运行。`;
        this.stopStatusPolling();
      } else if (error.response) {
        // 服务器返回了错误状态码
        this.errorMessage = `${operation}失败: 服务器返回错误 ${error.response.status}`;
      } else if (error.request) {
        // 请求已发出但没有收到响应
        this.backendConnected = false;
        this.errorMessage = `${operation}失败: 无法连接到服务器`;
        this.stopStatusPolling();
      } else {
        // 其他错误
        this.errorMessage = `${operation}失败: ${error.message}`;
      }
    },
    
    // 检查更新
    async checkForUpdates() {
      this.errorMessage = '';
      
      try {
        const response = await axios.post(`${this.apiBaseUrl}/check`, {}, { timeout: 10000 });
        if (response.data && response.data.success) {
          this.progressMessage = '正在检查更新...';
        } else {
          this.errorMessage = response.data.error || '检查更新失败';
        }
      } catch (error) {
        console.error('检查更新失败:', error);
        this.handleApiError(error, '检查更新');
      }
    },
    
    // 下载并应用更新
    async downloadAndApplyUpdate() {
      this.errorMessage = '';
      
      try {
        const response = await axios.post(`${this.apiBaseUrl}/apply`, {}, { timeout: 30000 });
        
        if (response.data && response.data.success) {
          this.progressMessage = '开始下载更新...';
        } else {
          this.errorMessage = response.data.error || '开始更新失败';
        }
      } catch (error) {
        console.error('开始更新失败:', error);
        this.handleApiError(error, '开始更新');
      }
    },
    
    // 回滚更新
    rollbackUpdate() {
      this.showRollbackDialog = true;
    },
    
    // 确认回滚
    async confirmRollback() {
      this.showRollbackDialog = false;
      this.errorMessage = '';
      
      try {
        const response = await axios.post(`${this.apiBaseUrl}/rollback`, {}, { timeout: 30000 });
        
        if (response.data && response.data.success) {
          this.progressMessage = '正在回滚...';
        } else {
          this.errorMessage = response.data.error || '开始回滚失败';
        }
      } catch (error) {
        console.error('开始回滚失败:', error);
        this.handleApiError(error, '开始回滚');
      }
    },
    
    // 取消回滚
    cancelRollback() {
      this.showRollbackDialog = false;
    }
  }
};
</script>

<style scoped>
.connection-status {
  padding: 10px;
  border-radius: 5px;
  margin-bottom: 15px;
}

.connection-status.success {
  background-color: rgba(76, 175, 80, 0.2);
  border: 1px solid rgba(76, 175, 80, 0.5);
  color: #4caf50;
}

.connection-status.error {
  background-color: rgba(244, 67, 54, 0.2);
  border: 1px solid rgba(244, 67, 54, 0.5);
  color: #f44336;
}

.current-version {
  background-color: rgba(255, 255, 255, 0.1);
  padding: 15px;
  border-radius: 5px;
  margin-bottom: 20px;
}

.error-message {
  color: #ff4444;
  font-weight: bold;
}

.action-buttons {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-bottom: 20px;
}

.left-button {
  width: 100%;
  margin-bottom: 10px;
}

.left-button.danger {
  background: rgba(255, 0, 0, 0.3);
  transition: all 0.2s ease;
}

.left-button.danger:hover:not(:disabled) {
  background: rgba(207, 0, 0, 0.3);
  transform: translateY(-1px);
}

.update-info {
  background-color: rgba(76, 175, 80, 0.2);
  padding: 15px;
  border-radius: 5px;
  margin-bottom: 20px;
  border: 1px solid rgba(76, 175, 80, 0.5);
}

.progress-container {
  margin: 20px 0;
}

.progress-bar {
  width: 100%;
  height: 20px;
  background-color: rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background-color: #4caf50;
  transition: width 0.3s;
}

.progress-text {
  text-align: center;
  margin-top: 5px;
  font-size: 14px;
  color: #eee;
}

.config-item {
  margin-bottom: 15px;
}

.config-item label {
  display: flex;
  align-items: center;
  cursor: pointer;
  color: #eee;
}

.config-item input {
  margin-right: 10px;
}

.config-help {
  display: block;
  font-size: 12px;
  color: #aaa;
  margin-top: 5px;
}

.dialog-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-color: rgba(0, 0, 0, 0.7);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 1000;
}

.dialog {
  background-color: rgba(50, 50, 50, 0.9);
  padding: 20px;
  border-radius: 8px;
  max-width: 400px;
  width: 90%;
  border: 1px solid #555;
  backdrop-filter: blur(10px);
}

.dialog h3 {
  margin-top: 0;
  color: #eee;
}

.dialog p {
  color: #ddd;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 20px;
}

@media (max-width: 600px) {
  .action-buttons {
    flex-direction: column;
  }
  
  .left-button {
    width: 100%;
  }
}
</style>