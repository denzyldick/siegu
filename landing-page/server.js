const express = require('express');
const fs = require('fs');
const path = require('path');
const app = express();
const PORT = process.env.PORT || 80;

app.use(express.json());
app.use(express.static(path.join(__dirname)));

const DATA_DIR = path.join(__dirname, 'data');
const WAITLIST_FILE = path.join(DATA_DIR, 'waitlist.csv');
const DOWNLOAD_INTENT_FILE = path.join(DATA_DIR, 'download-intents.csv');

// Ensure data directory exists
if (!fs.existsSync(DATA_DIR)) {
    fs.mkdirSync(DATA_DIR);
}

// Ensure CSV exists with headers
if (!fs.existsSync(WAITLIST_FILE)) {
    fs.writeFileSync(WAITLIST_FILE, 'timestamp,email,os,source\n');
}

if (!fs.existsSync(DOWNLOAD_INTENT_FILE)) {
    fs.writeFileSync(DOWNLOAD_INTENT_FILE, 'timestamp,os,source,userAgent\n');
}

app.post('/api/waitlist', (req, res) => {
    const { email, os, source } = req.body;
    if (!email || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
        return res.status(400).send('Valid email is required');
    }

    const timestamp = new Date().toISOString();
    const row = [
        timestamp,
        email,
        os || 'unknown',
        source || 'unknown'
    ].map(csvEscape).join(',') + '\n';

    fs.appendFile(WAITLIST_FILE, row, (err) => {
        if (err) {
            console.error('Failed to save to waitlist:', err);
            return res.status(500).send('Internal server error');
        }
        console.log(`New signup: ${email} (${os || 'unknown'}, ${source || 'unknown'})`);
        res.status(200).send('Successfully joined the waitlist');
    });
});

app.post('/api/download-intent', express.text({ type: '*/*' }), (req, res) => {
    let payload = {};
    try {
        payload = JSON.parse(req.body || '{}');
    } catch (err) {
        payload = {};
    }

    const timestamp = new Date().toISOString();
    const row = [
        timestamp,
        payload.os || 'unknown',
        payload.source || 'unknown',
        req.get('user-agent') || 'unknown'
    ].map(csvEscape).join(',') + '\n';

    fs.appendFile(DOWNLOAD_INTENT_FILE, row, (err) => {
        if (err) {
            console.error('Failed to save download intent:', err);
            return res.status(500).send('Internal server error');
        }
        res.status(204).end();
    });
});

function csvEscape(value) {
    const str = String(value);
    if (/[",\n\r]/.test(str)) {
        return `"${str.replace(/"/g, '""')}"`;
    }
    return str;
}

app.listen(PORT, '0.0.0.0', () => {
    console.log(`Siegu Landing Page Server running on port ${PORT}`);
});
