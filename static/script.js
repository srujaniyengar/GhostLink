// --- State Management ---
const state = {
    fullAddress: null,
    peerAddress: null,
    connectionStatus: 'disconnected', // disconnected, punching, connected
    isIpValid: false,
    isPortValid: false,
    sseSource: null,
    lastSavedMessage: null, // For storing the message on timeout
};

// --- DOM Elements ---
const els = {
    // Views
    viewHome: document.getElementById('view-home'),
    viewPunching: document.getElementById('view-punching'),
    viewConnected: document.getElementById('view-connected'),

    // Shared
    statusText: document.getElementById('statusText'),
    statusBadge: document.getElementById('statusBadge'),
    statusDot: document.querySelector('#statusBadge .status-dot'),

    // Home
    myIpDisplay: document.getElementById('myIpDisplay'),
    apiErrorMsg: document.getElementById('apiErrorMsg'),
    copyBtn: document.getElementById('copyBtn'),
    refreshBtn: document.getElementById('refreshBtn'),
    connectForm: document.getElementById('connectForm'),
    peerIpInput: document.getElementById('peerIp'),
    peerPortInput: document.getElementById('peerPort'),
    ipError: document.getElementById('ipError'),
    portError: document.getElementById('portError'),
    submitBtn: document.querySelector('#connectForm button'),

    // Punching / Visualization
    vizClientIp: document.getElementById('vizClientIp'),
    vizPeerIp: document.getElementById('vizPeerIp'),
    punchLogs: document.getElementById('punchLogs'),
    punchTimeout: document.getElementById('punchTimeout'),

    // Connected
    connLocalIp: document.getElementById('connLocalIp'),
    connRemoteIp: document.getElementById('connRemoteIp'),
    disconnectBtn: document.getElementById('disconnectBtn'),

    // Toast
    toast: document.getElementById('toast'),
    toastMsg: document.querySelector('#toast .toast-msg'),
};

// --- Initialization ---
async function init() {
    toggleSubmitButton();
    await fetchPublicInfo();
    
    // Initial status check to set UI state immediately (prevents flicker)
    await fetchInitialStatus();
    
    // Connect to SSE for real-time updates
    connectSSE();

    setupEventListeners();
}

// --- Status & State Logic ---

async function fetchInitialStatus() {
    try {
        const res = await fetch('/api/status');
        const data = await res.json();
        // The /api/status endpoint returns AppState
        // { public_ip: "...", status: "...", peer_ip: "..." }
        if(data && data.status) {
            handleStatusChange(data.status, data);
        }
    } catch (e) {
        console.warn("Initial status fetch failed", e);
    }
}

async function fetchPeerInfo() {
    // Fallback: If we don't have peer info from events, try to fetch it explicitly
    try {
        const res = await fetch('/api/peer');
        if(res.ok) {
            const data = await res.json();
            // Expecting { ip: "1.2.3.4", port: 1234 } or string
            if (data.ip && data.port) {
                state.peerAddress = `${data.ip}:${data.port}`;
            } else if (typeof data === 'string') {
                state.peerAddress = data;
            }
        }
    } catch(e) {
        console.log("No peer info available via API");
    }
}

function handleStatusChange(statusStr, data = {}) {
    // Backend enum variants might be "Disconnected", "Punching", etc.
    // SSE event tag renames them to "DISCONNECTED", "PUNCHING", etc.
    // We normalize to upper case for comparison.
    const normStatus = (statusStr || 'DISCONNECTED').toUpperCase();
    
    // Handle Data Syncing based on Event Structure
    // 1. If it's a DISCONNECTED event (AppEvent::Disconnected { state }), it has a nested `state` object.
    if (normStatus === 'DISCONNECTED' && data.state) {
        if (data.state.public_ip) state.fullAddress = data.state.public_ip;
        if (data.state.peer_ip) state.peerAddress = data.state.peer_ip;
        // We could also sync nat_type here if needed
    }
    // 2. If it's a standard AppState (from /api/status), fields are top-level.
    else {
        if (data.public_ip) state.fullAddress = data.public_ip;
        if (data.peer_ip) state.peerAddress = data.peer_ip;
    }

    // Logic: Transitioning FROM Punching TO Disconnected
    if (state.connectionStatus === 'punching' && normStatus === 'DISCONNECTED') {
        if (state.lastSavedMessage) {
            showToast(state.lastSavedMessage);
            state.lastSavedMessage = null; // Clear after showing
        }
    }

    state.connectionStatus = normStatus.toLowerCase();
    
    // 1. Update Badge
    renderStatusBadge();

    // 2. Switch Views & Logic
    els.viewHome.classList.remove('active');
    els.viewPunching.classList.remove('active');
    els.viewConnected.classList.remove('active');

    if (normStatus === 'PUNCHING') {
        enterPunchingState(data);
    } else if (normStatus === 'CONNECTED') {
        enterConnectedState(data);
    } else {
        // DISCONNECTED
        els.viewHome.classList.add('active');
        // We generally keep peerAddress in state until a new connection clears it,
        // or if the backend explicitly sends null in the state.
        if (data.state && data.state.peer_ip === null) {
            state.peerAddress = null;
        }
    }
}

async function enterPunchingState(data) {
    els.viewPunching.classList.add('active');
    
    // If peer info is missing, try to fetch it
    if (!state.peerAddress) {
        await fetchPeerInfo();
    }
    
    els.vizClientIp.innerText = state.fullAddress || "Unknown";
    els.vizPeerIp.innerText = state.peerAddress || "Target";

    // Handle Timeout Display (from AppEvent::Punching { timeout })
    if (data.timeout !== undefined && data.timeout !== null) {
        els.punchTimeout.innerText = `${data.timeout}s`;
        
        // "when the time left is 0, save the message"
        if (data.timeout === 0 && data.message) {
            state.lastSavedMessage = data.message;
        }
    }

    // Handle Logs (from AppEvent::Punching { message })
    if (data.message) {
        addLog(data.message);
    }
}

async function enterConnectedState(data) {
    els.viewConnected.classList.add('active');
    
    if (!state.peerAddress) {
        await fetchPeerInfo();
    }

    els.connLocalIp.innerText = state.fullAddress;
    els.connRemoteIp.innerText = state.peerAddress || "Connected Peer";

    // AppEvent::Connected { message }
    if (data.message) {
        // Optionally log or toast the success message
        console.log("Connected:", data.message);
    }
}

// --- SSE (Real-time Events) ---
function connectSSE() {
    if (state.sseSource && state.sseSource.readyState !== EventSource.CLOSED) {
        return; 
    }

    // Endpoint: /api/events
    state.sseSource = new EventSource('/api/events');

    state.sseSource.onmessage = (event) => {
        try {
            const data = JSON.parse(event.data);
            
            // Structure: 
            // { status: "DISCONNECTED", state: { ... } }
            // { status: "PUNCHING", timeout: 10, message: "..." }
            // { status: "CONNECTED", message: "..." }

            if (data.status) {
                handleStatusChange(data.status, data);
            }
        } catch (e) {
            console.error("SSE Parse Error", e);
        }
    };

    state.sseSource.onerror = (err) => {
        console.warn("SSE Connection issue", err);
        // Browser will auto-retry
    };
}

// --- UI Helpers ---
function renderStatusBadge() {
    const s = state.connectionStatus;
    els.statusText.innerText = s.charAt(0).toUpperCase() + s.slice(1);
    
    let color, bg, border;
    if (s === 'connected') {
        color = 'var(--success)'; bg = 'rgba(16, 185, 129, 0.1)'; border = 'rgba(16, 185, 129, 0.2)';
    } else if (s === 'punching') {
        color = '#f59e0b'; bg = 'rgba(245, 158, 11, 0.1)'; border = 'rgba(245, 158, 11, 0.2)';
    } else {
        color = 'var(--danger)'; bg = 'rgba(239, 68, 68, 0.1)'; border = 'rgba(239, 68, 68, 0.2)';
    }

    els.statusDot.style.backgroundColor = color;
    els.statusDot.style.boxShadow = `0 0 8px ${color}`;
    els.statusBadge.style.color = color;
    els.statusBadge.style.background = bg;
    els.statusBadge.style.borderColor = border;
}

function addLog(message) {
    const row = document.createElement('div');
    row.className = `log-line system`; 
    // Basic timestamp
    const timeStr = new Date().toLocaleTimeString('en-US', {hour12: false, hour:"2-digit", minute:"2-digit", second:"2-digit"});
    
    row.innerHTML = `<span class="log-timestamp">[${timeStr}]</span> ${message}`;
    els.punchLogs.appendChild(row);
    els.punchLogs.scrollTop = els.punchLogs.scrollHeight;
}

// --- API Calls ---
async function fetchPublicInfo() {
    els.refreshBtn.classList.add('spin');
    els.refreshBtn.disabled = true;
    els.myIpDisplay.style.opacity = '0.5';

    try {
        const res = await fetch('/api/ip');
        if (!res.ok) throw new Error(`Server error`);
        const data = await res.json();
        // Assuming { public_ip: "..." }
        if (data.public_ip) {
            state.fullAddress = data.public_ip;
            renderMyInfo(true);
        } else {
            renderMyInfo(false);
        }
    } catch (err) {
        renderMyInfo(false);
    } finally {
        setTimeout(() => {
            els.refreshBtn.classList.remove('spin');
            els.refreshBtn.disabled = false;
            els.myIpDisplay.style.opacity = '1';
        }, 500);
    }
}

function renderMyInfo(success) {
    if (success && state.fullAddress) {
        els.myIpDisplay.innerText = state.fullAddress;
        els.myIpDisplay.classList.remove('error');
        els.apiErrorMsg.style.display = 'none';
        els.copyBtn.style.display = 'flex';
    } else {
        els.myIpDisplay.innerText = "Connection Failed";
        els.myIpDisplay.classList.add('error');
        els.apiErrorMsg.innerText = "Could not reach node.";
        els.apiErrorMsg.style.display = 'block';
        els.copyBtn.style.display = 'none';
    }
}

async function handleConnect(e) {
    e.preventDefault();
    if (!state.isIpValid || !state.isPortValid) return;

    const ip = els.peerIpInput.value.trim();
    const port = parseInt(els.peerPortInput.value.trim(), 10);
    // Optimistically set peer address locally
    state.peerAddress = `${ip}:${port}`;

    const btn = els.submitBtn;
    btn.innerText = "Initiating...";
    btn.disabled = true;

    try {
        const res = await fetch('/api/connect', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ ip, port })
        });
        if (!res.ok) throw new Error();
        
        // Logic handled by SSE events now
        els.punchLogs.innerHTML = '';
        state.lastSavedMessage = null;

    } catch (err) {
        showToast("Connection failed to start");
    } finally {
        btn.innerText = "Establish Link";
        btn.disabled = false;
    }
}

async function handleDisconnect() {
    try {
        await fetch('/api/disconnect', { method: 'POST' });
    } catch(e) {
        console.error(e);
    }
}

// --- Validation & Utilities ---
function toggleSubmitButton() {
    els.submitBtn.disabled = !(state.isIpValid && state.isPortValid);
}

function showToast(message) {
    els.toastMsg.textContent = message;
    els.toast.classList.add('show');
    setTimeout(() => els.toast.classList.remove('show'), 3000);
}

function copyToClipboard() {
    if (state.fullAddress) {
        // Use legacy execCommand as requested/safer for iframes
        const textarea = document.createElement('textarea');
        textarea.value = state.fullAddress;
        document.body.appendChild(textarea);
        textarea.select();
        try {
            document.execCommand('copy');
            showToast("Copied to clipboard");
        } catch (err) {
            console.error('Fallback copy failed', err);
        }
        document.body.removeChild(textarea);
    }
}

const validators = {
    ip: (ip) => /^(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$/.test(ip),
    port: (p) => { const n = parseInt(p, 10); return !isNaN(n) && n > 0 && n <= 65535; }
};

function handleIpValidation(eventType) {
    const val = els.peerIpInput.value.trim();
    const isValid = validators.ip(val);
    state.isIpValid = isValid;
    if (isValid) {
        els.peerIpInput.classList.remove('error', 'valid');
        els.peerIpInput.classList.add('valid');
        els.ipError.style.display = 'none';
    } else {
        els.peerIpInput.classList.remove('valid');
        if (eventType === 'blur' && val.length > 0) {
            els.peerIpInput.classList.add('error');
            els.ipError.style.display = 'block';
        } else {
            els.peerIpInput.classList.remove('error');
            els.ipError.style.display = 'none';
        }
    }
    toggleSubmitButton();
}

function handlePortValidation() {
    const val = els.peerPortInput.value.trim();
    const isValid = validators.port(val);
    state.isPortValid = isValid;
    if (isValid) {
        els.peerPortInput.classList.remove('error', 'valid');
        els.peerPortInput.classList.add('valid');
        els.portError.style.display = 'none';
    } else {
        els.peerPortInput.classList.remove('valid');
        if (val.length > 0) {
            els.peerPortInput.classList.add('error');
            els.portError.style.display = 'block';
        } else {
            els.peerPortInput.classList.remove('error');
            els.portError.style.display = 'none';
        }
    }
    toggleSubmitButton();
}

function setupEventListeners() {
    if(els.copyBtn) els.copyBtn.addEventListener('click', copyToClipboard);
    if(els.refreshBtn) els.refreshBtn.addEventListener('click', fetchPublicInfo);
    els.connectForm.addEventListener('submit', handleConnect);
    els.disconnectBtn.addEventListener('click', handleDisconnect);
    els.peerIpInput.addEventListener('input', () => handleIpValidation('input'));
    els.peerIpInput.addEventListener('blur', () => handleIpValidation('blur'));
    els.peerPortInput.addEventListener('input', handlePortValidation);
}

init();
