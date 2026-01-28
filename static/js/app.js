function loadData() {
    loadStats();
    loadFiles();
    loadSymbols();
}

function loadStats() {
    fetch('/api/stats')
        .then(response => response.json())
        .then(data => {
            document.getElementById('stats').innerHTML = `
                <h3>Database Statistics</h3>
                <p><strong>Files:</strong> ${data.files}</p>
                <p><strong>Symbols:</strong> ${data.symbols}</p>
                <p><strong>Edges:</strong> ${data.edges}</p>
            `;
        })
        .catch(error => {
            console.error('Error loading stats:', error);
            document.getElementById('stats').innerHTML = '<div class="error">Error loading statistics</div>';
        });
}

function loadFiles() {
    document.getElementById('files').innerHTML = '<div class="loading">Loading files...</div>';
    
    fetch('/api/files')
        .then(response => response.json())
        .then(data => {
            let html = '<table><tr><th>Path</th><th>Language</th><th>Category</th><th>Symbols</th></tr>';
            data.forEach(file => {
                html += `<tr><td>${file.path}</td><td>${file.lang}</td><td>${file.category}</td><td>${file.symbol_count}</td></tr>`;
            });
            html += '</table>';
            document.getElementById('files').innerHTML = html;
        })
        .catch(error => {
            console.error('Error loading files:', error);
            document.getElementById('files').innerHTML = '<div class="error">Error loading files</div>';
        });
}

function loadSymbols() {
    document.getElementById('symbols').innerHTML = '<div class="loading">Loading symbols...</div>';
    
    fetch('/api/symbols')
        .then(response => response.json())
        .then(data => {
            let html = '<table><tr><th>Name</th><th>Kind</th><th>File</th><th>Category</th></tr>';
            data.forEach(symbol => {
                html += `<tr><td>${symbol.name}</td><td>${symbol.kind}</td><td>${symbol.file_path}</td><td>${symbol.category}</td></tr>`;
            });
            html += '</table>';
            document.getElementById('symbols').innerHTML = html;
        })
        .catch(error => {
            console.error('Error loading symbols:', error);
            document.getElementById('symbols').innerHTML = '<div class="error">Error loading symbols</div>';
        });
}

function searchSymbols() {
    const query = document.getElementById('searchInput').value;
    document.getElementById('symbols').innerHTML = '<div class="loading">Searching...</div>';
    
    fetch(`/api/symbols?search=${encodeURIComponent(query)}`)
        .then(response => response.json())
        .then(data => {
            let html = '<table><tr><th>Name</th><th>Kind</th><th>File</th><th>Category</th></tr>';
            data.forEach(symbol => {
                html += `<tr><td>${symbol.name}</td><td>${symbol.kind}</td><td>${symbol.file_path}</td><td>${symbol.category}</td></tr>`;
            });
            html += '</table>';
            document.getElementById('symbols').innerHTML = html;
        })
        .catch(error => {
            console.error('Error searching symbols:', error);
            document.getElementById('symbols').innerHTML = '<div class="error">Error searching symbols</div>';
        });
}

// Load data when page loads
document.addEventListener('DOMContentLoaded', loadData);
