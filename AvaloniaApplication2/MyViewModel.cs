using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using Avalonia;
using DynamicData;

namespace AvaloniaApplication2;

class MyClass
{
    public required string Id { get; set; }
}

internal class MyViewModel : AvaloniaObject
{
    public MyViewModel()
    {
        _cache.Connect()
            .StartWithEmpty()
            .Bind(out _collection)
            .Subscribe()
            //.DisposeWith(this)
            ;

        ((INotifyCollectionChanged)Collection).CollectionChanged += CollectionChanged;
    }

    static void CollectionChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {

    }

    public ReadOnlyObservableCollection<MyClass> Collection => _collection;
    readonly ReadOnlyObservableCollection<MyClass> _collection;
    readonly SourceCache<MyClass, string> _cache = new(m => m.Id);
    public void AddOrUpdate(MyClass monitor)
    {
        _cache.AddOrUpdate(monitor);
    }
}