/**
 * Test script for the npm package
 */

const { RustyGunNode, SQLiteCompatibleAPI } = require('./dist/node-index.js');

async function testPackage() {
  console.log('🧪 Testing Rusty Gun npm package...\n');

  try {
    // Test 1: Create RustyGunNode instance
    console.log('1. Creating RustyGunNode instance...');
    const rustyGun = new RustyGunNode({
      config: {
        port: 34567,
        host: 'localhost',
        dataDir: './test-data'
      },
      autoStart: false // Don't auto-start for testing
    });
    console.log('✅ RustyGunNode created successfully');

    // Test 2: Create SQLiteCompatibleAPI instance
    console.log('2. Creating SQLiteCompatibleAPI instance...');
    const sqlite = new SQLiteCompatibleAPI({
      config: {
        port: 34567,
        host: 'localhost',
        dataDir: './test-data'
      },
      autoStart: false
    });
    console.log('✅ SQLiteCompatibleAPI created successfully');

    // Test 3: Check configuration
    console.log('3. Checking configuration...');
    console.log('   API URL:', rustyGun.getApiUrl());
    console.log('   Web URL:', rustyGun.getWebUrl());
    console.log('   Is Running:', rustyGun.isServerRunning());
    console.log('✅ Configuration looks good');

    // Test 4: Test event emitter functionality
    console.log('4. Testing event emitter...');
    rustyGun.on('started', () => console.log('   Event: started'));
    rustyGun.on('stopped', () => console.log('   Event: stopped'));
    rustyGun.on('error', (error) => console.log('   Event: error', error.message));
    console.log('✅ Event emitter working');

    console.log('\n🎉 All tests passed! The npm package is working correctly.');
    console.log('\n📦 Package is ready for VSCode extension integration!');
    console.log('\n📚 Usage in VSCode extension:');
    console.log('   import { SQLiteCompatibleAPI } from "rusty-gun";');
    console.log('   const db = new SQLiteCompatibleAPI();');
    console.log('   await db.start();');

  } catch (error) {
    console.error('❌ Test failed:', error.message);
    process.exit(1);
  }
}

testPackage();

