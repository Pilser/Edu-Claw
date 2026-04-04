/* educlaw-web/api.js
 *
 * REST helpers + login-form submit handler.
 */

const API = {
  /**
   * Authenticate a student.
   * Returns { token, student_id, display_name, role }
   */
  async login(username, password) {
    const res = await fetch('/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(text || `HTTP ${res.status}`);
    }
    return res.json();
  },

  /**
   * Create a new session for the authenticated student.
   * Returns { session_id, ws_url, token }
   */
  async createSession(studentId, token, course, assignment) {
    const body = { student_id: studentId, course };
    if (assignment) body.assignment = assignment;

    const res = await fetch('/sessions', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${token}`,
      },
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(text || `HTTP ${res.status}`);
    }
    return res.json();
  },
};

// ── Login form handler ────────────────────────────────────────────────────

document.addEventListener('DOMContentLoaded', () => {
  const form    = document.getElementById('login-form');
  const errorEl = document.getElementById('login-error');
  const btn     = form.querySelector('button[type="submit"]');

  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    errorEl.textContent = '';
    btn.disabled = true;
    btn.textContent = 'Connecting…';

    const username   = document.getElementById('username').value.trim();
    const password   = document.getElementById('password').value;
    const course     = document.getElementById('course').value.trim();
    const assignment = document.getElementById('assignment').value.trim() || undefined;

    try {
      const auth = await API.login(username, password);
      const sess = await API.createSession(auth.student_id, auth.token, course, assignment);
      window.startSession(sess.session_id, sess.token, auth.display_name);
    } catch (err) {
      errorEl.textContent = 'Error: ' + err.message;
      btn.disabled = false;
      btn.textContent = 'Start Session';
    }
  });
});
